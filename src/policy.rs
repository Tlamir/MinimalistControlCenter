//! OFFLINE SIMULATION ONLY. Unitless demand is not PWM, RPM or a hardware command.
//! There is deliberately no connection from these decisions to a hardware backend.
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub celsius: f64,
    pub demand: u8,
}
#[derive(Debug, Clone)]
pub struct Curve {
    points: Vec<Point>,
}
impl Curve {
    pub fn new(points: Vec<Point>) -> Result<Self, &'static str> {
        if !(2..=16).contains(&points.len()) {
            return Err("A curve needs 2–16 points");
        }
        for p in &points {
            if !p.celsius.is_finite()
                || !(-20.0..=120.0).contains(&p.celsius)
                || !(20..=100).contains(&p.demand)
            {
                return Err("Invalid temperature or demand");
            }
        }
        if points
            .windows(2)
            .any(|w| w[0].celsius >= w[1].celsius || w[0].demand > w[1].demand)
        {
            return Err("Curve must increase in temperature and never decrease in demand");
        }
        Ok(Self { points })
    }
    fn demand(&self, temp: f64) -> u8 {
        if temp <= self.points[0].celsius {
            return self.points[0].demand;
        }
        for pair in self.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if temp <= b.celsius {
                let fraction = (temp - a.celsius) / (b.celsius - a.celsius);
                return (f64::from(a.demand) + fraction * f64::from(b.demand - a.demand)).ceil()
                    as u8;
            }
        }
        self.points.last().expect("validated nonempty curve").demand
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub celsius: f64,
    pub captured_ms: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "recommendation", rename_all = "snake_case")]
pub enum Decision {
    FirmwareAutomaticRequired { reason: &'static str },
    EmergencyCoolingRequired,
    CoolingDemand { unitless_percent: u8 },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub cpu: Curve,
    pub gpu: Curve,
    pub cpu_emergency_c: f64,
    pub gpu_emergency_c: f64,
    pub max_age_ms: u64,
    pub decrease_delay_ms: u64,
    pub hysteresis_c: f64,
    pub max_decrease: u8,
}
impl Config {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.cpu_emergency_c.is_finite()
            || !self.gpu_emergency_c.is_finite()
            || !(50.0..=110.0).contains(&self.cpu_emergency_c)
            || !(50.0..=100.0).contains(&self.gpu_emergency_c)
        {
            return Err("Invalid illustrative emergency limits");
        }
        if !(100..=2000).contains(&self.max_age_ms)
            || !(1000..=30_000).contains(&self.decrease_delay_ms)
            || !self.hysteresis_c.is_finite()
            || !(1.0..=10.0).contains(&self.hysteresis_c)
            || !(1..=10).contains(&self.max_decrease)
        {
            return Err("Invalid simulation timing");
        }
        Ok(())
    }
}

pub struct Simulator {
    config: Config,
    last_time: Option<u64>,
    demand: Option<u8>,
    lower_since: Option<u64>,
}
impl Simulator {
    pub fn new(config: Config) -> Result<Self, &'static str> {
        config.validate()?;
        Ok(Self {
            config,
            last_time: None,
            demand: None,
            lower_since: None,
        })
    }
    fn fail(&mut self, reason: &'static str) -> Decision {
        self.demand = None;
        self.lower_since = None;
        Decision::FirmwareAutomaticRequired { reason }
    }
    pub fn evaluate(&mut self, now_ms: u64, cpu: Option<Sample>, gpu: Option<Sample>) -> Decision {
        if self.last_time.is_some_and(|last| now_ms <= last) {
            return self.fail("non_increasing_clock");
        }
        self.last_time = Some(now_ms);
        let valid = |s: Sample| {
            s.celsius.is_finite()
                && (-20.0..=130.0).contains(&s.celsius)
                && now_ms
                    .checked_sub(s.captured_ms)
                    .is_some_and(|age| age <= self.config.max_age_ms)
        };
        // A trustworthy hot reading overrides another missing/bad sensor.
        if cpu.is_some_and(|s| valid(s) && s.celsius >= self.config.cpu_emergency_c)
            || gpu.is_some_and(|s| valid(s) && s.celsius >= self.config.gpu_emergency_c)
        {
            self.demand = Some(100);
            self.lower_since = None;
            return Decision::EmergencyCoolingRequired;
        }
        let (Some(cpu), Some(gpu)) = (cpu, gpu) else {
            return self.fail("missing_sensor");
        };
        if !valid(cpu) || !valid(gpu) {
            return self.fail("invalid_or_stale_sensor");
        }
        let raw = self
            .config
            .cpu
            .demand(cpu.celsius)
            .max(self.config.gpu.demand(gpu.celsius));
        let Some(current) = self.demand else {
            self.demand = Some(raw);
            return Decision::CoolingDemand {
                unitless_percent: raw,
            };
        };
        if raw >= current {
            self.demand = Some(raw);
            self.lower_since = None;
            return Decision::CoolingDemand {
                unitless_percent: raw,
            };
        }
        let buffered = self
            .config
            .cpu
            .demand(cpu.celsius + self.config.hysteresis_c)
            .max(
                self.config
                    .gpu
                    .demand(gpu.celsius + self.config.hysteresis_c),
            );
        if buffered >= current {
            self.lower_since = None;
            return Decision::CoolingDemand {
                unitless_percent: current,
            };
        }
        let since = *self.lower_since.get_or_insert(now_ms);
        if now_ms - since >= self.config.decrease_delay_ms {
            let next = buffered.max(current.saturating_sub(self.config.max_decrease));
            self.demand = Some(next);
            self.lower_since = Some(now_ms);
            Decision::CoolingDemand {
                unitless_percent: next,
            }
        } else {
            Decision::CoolingDemand {
                unitless_percent: current,
            }
        }
    }
}

/// Fictional tuning fixture. These are NOT validated settings for any laptop.
pub fn example() -> Simulator {
    let curve = Curve::new(vec![
        Point {
            celsius: 35.0,
            demand: 30,
        },
        Point {
            celsius: 60.0,
            demand: 50,
        },
        Point {
            celsius: 80.0,
            demand: 100,
        },
    ])
    .unwrap();
    Simulator::new(Config {
        cpu: curve.clone(),
        gpu: curve,
        cpu_emergency_c: 90.0,
        gpu_emergency_c: 85.0,
        max_age_ms: 1500,
        decrease_delay_ms: 5000,
        hysteresis_c: 3.0,
        max_decrease: 5,
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(t: f64, ms: u64) -> Option<Sample> {
        Some(Sample {
            celsius: t,
            captured_ms: ms,
        })
    }
    fn demand(d: Decision) -> u8 {
        match d {
            Decision::CoolingDemand { unitless_percent } => unitless_percent,
            _ => panic!("expected demand"),
        }
    }
    #[test]
    fn rejects_malformed_curves() {
        for points in [
            vec![],
            vec![Point {
                celsius: 30.,
                demand: 30,
            }],
            vec![
                Point {
                    celsius: 30.,
                    demand: 30,
                },
                Point {
                    celsius: 30.,
                    demand: 40,
                },
            ],
            vec![
                Point {
                    celsius: 30.,
                    demand: 50,
                },
                Point {
                    celsius: 60.,
                    demand: 30,
                },
            ],
            vec![
                Point {
                    celsius: f64::NAN,
                    demand: 30,
                },
                Point {
                    celsius: 60.,
                    demand: 40,
                },
            ],
            vec![
                Point {
                    celsius: 30.,
                    demand: 0,
                },
                Point {
                    celsius: 60.,
                    demand: 101,
                },
            ],
        ] {
            assert!(Curve::new(points).is_err());
        }
    }
    #[test]
    fn all_valid_temperatures_produce_bounded_monotonic_demand() {
        let sim = example();
        let mut last = 0;
        for i in -200..=1200 {
            let value = sim.config.cpu.demand(f64::from(i) / 10.);
            assert!((20..=100).contains(&value));
            assert!(value >= last);
            last = value;
        }
    }
    #[test]
    fn gpu_demand_is_not_ignored() {
        let mut a = example();
        let low = demand(a.evaluate(0, s(40., 0), s(40., 0)));
        let high = demand(a.evaluate(1000, s(40., 1000), s(75., 1000)));
        assert!(high > low);
    }
    #[test]
    fn increase_is_immediate() {
        let mut a = example();
        let low = demand(a.evaluate(0, s(40., 0), s(40., 0)));
        let high = demand(a.evaluate(1, s(80., 1), s(40., 1)));
        assert!(high > low);
    }
    #[test]
    fn emergency_bypasses_smoothing() {
        let mut a = example();
        a.evaluate(0, s(40., 0), s(40., 0));
        assert_eq!(
            a.evaluate(1, s(90., 1), s(40., 1)),
            Decision::EmergencyCoolingRequired
        );
    }
    #[test]
    fn valid_emergency_wins_over_missing_sensor() {
        assert_eq!(
            example().evaluate(1, None, s(85., 1)),
            Decision::EmergencyCoolingRequired
        );
    }
    #[test]
    fn invalid_or_missing_readings_never_mean_cold() {
        for temp in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -21., 131.] {
            assert!(matches!(
                example().evaluate(1, s(temp, 1), s(40., 1)),
                Decision::FirmwareAutomaticRequired { .. }
            ));
        }
        assert!(matches!(
            example().evaluate(1, None, s(40., 1)),
            Decision::FirmwareAutomaticRequired { .. }
        ));
    }
    #[test]
    fn stale_future_and_reset_clock_fail_closed() {
        assert!(matches!(
            example().evaluate(2000, s(40., 0), s(40., 2000)),
            Decision::FirmwareAutomaticRequired { .. }
        ));
        assert!(matches!(
            example().evaluate(1, s(40., 2), s(40., 1)),
            Decision::FirmwareAutomaticRequired { .. }
        ));
        let mut a = example();
        a.evaluate(100, s(40., 100), s(40., 100));
        assert!(matches!(
            a.evaluate(99, s(40., 99), s(40., 99)),
            Decision::FirmwareAutomaticRequired { .. }
        ));
        assert!(matches!(
            a.evaluate(100, s(40., 100), s(40., 100)),
            Decision::FirmwareAutomaticRequired { .. }
        ));
    }
    #[test]
    fn decreases_wait_and_are_rate_limited() {
        let mut a = example();
        assert_eq!(demand(a.evaluate(0, s(80., 0), s(40., 0))), 100);
        assert_eq!(demand(a.evaluate(1000, s(40., 1000), s(40., 1000))), 100);
        assert_eq!(demand(a.evaluate(5999, s(40., 5999), s(40., 5999))), 100);
        assert_eq!(demand(a.evaluate(6000, s(40., 6000), s(40., 6000))), 95);
        assert_eq!(demand(a.evaluate(6001, s(40., 6001), s(40., 6001))), 95);
    }
    #[test]
    fn small_temperature_jitter_does_not_reduce_speed() {
        let mut a = example();
        let start = demand(a.evaluate(0, s(70., 0), s(40., 0)));
        for t in 1..20 {
            assert_eq!(
                demand(a.evaluate(t * 1000, s(69., t * 1000), s(40., t * 1000))),
                start
            );
        }
    }
    #[test]
    fn large_time_values_do_not_wrap() {
        let mut a = example();
        a.evaluate(u64::MAX - 1, s(40., u64::MAX - 1), s(40., u64::MAX - 1));
        assert!(matches!(
            a.evaluate(u64::MAX, s(40., u64::MAX), s(40., u64::MAX)),
            Decision::CoolingDemand { .. }
        ));
    }
    #[test]
    fn deterministic_fault_injection_stays_bounded() {
        let mut a = example();
        let mut seed = 17u64;
        for i in 0..100_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let t = (seed % 180) as f64 - 30.;
            let cpu = if seed.is_multiple_of(17) {
                None
            } else {
                s(t, i * 500)
            };
            if let Decision::CoolingDemand { unitless_percent } =
                a.evaluate(i * 500, cpu, s(45., i * 500))
            {
                assert!((20..=100).contains(&unitless_percent));
            }
        }
    }
}
