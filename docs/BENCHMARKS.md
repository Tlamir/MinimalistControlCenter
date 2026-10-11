# Benchmarks

![Original control center vs. Minimalist: RAM, CPU and idle window reply](../assets/benchmarks-idle-cpu-1pct.svg)

| App / state | RAM, resident | CPU, whole PC | Idle reply, p95 |
|---|---:|---:|---:|
| Original / open | 175.8 MiB | 0.004-0.017% | 0.329-0.344 ms |
| Minimalist 0.8.0 / open | 25.4 MiB | 0.034-0.073% | 0.289-0.328 ms |
| Original / minimized | 171.8 MiB | 0.004% | 0.319-0.341 ms |
| Minimalist 0.8.0 / tray | 14.1 MiB | 0% observed | Not measured |

Minimalist used less RAM in these sessions. The original app's idle default page used less CPU than Minimalist's connected Cooling page. Different pages and power settings mean this is not a controlled comparison of speed or energy use. Battery life was not measured.

The CPU chart uses a fixed 0-1% scale, with exact labels: both open averages were below 0.1%. Minimized/tray rows show background idle usage. Minimalist's observed zero is below counter resolution, not a guarantee of no CPU activity.

## Method

Same Monster Tulpar T5 V17.1 / N.1.06, i7-8750H, GTX 1060, 16 GB RAM, 12 logical processors; Windows 25H2 build 26200.9457. One app ran at a time with normal desktop activity and no build or stress test.

- **Original:** October 10, AC / Power Saver, default page; two 30-second runs each while open and minimized. Includes its GUI, tray and launcher service.
- **Minimalist 0.8.0:** October 11, battery / Balanced, connected Cooling page; two 30-second open runs and one 30-second tray run. Includes the GUI, transient children and read-only observer. No hardware writes during measurement.
- **RAM:** mean of per-run median aggregate working sets, sampled once a second. Shared pages can count twice; short helper peaks can be missed.
- **CPU:** process time / elapsed time / 12 logical processors. Shared WMI/Windows/driver hosts and the benchmark process are excluded. Zero means below counter resolution.
- **Response:** p95 idle `WM_NULL` reply time, not click latency, frame rate or intermittent hiccups.

[Original app data](../reports/benchmarks/v0.7.0-2026-10-10.json) | [JSON report](../reports/benchmarks/v0.8.0-2026-10-11.json) | [Measurement script](../scripts/measure-app.ps1) | [Chart script](../scripts/plot_benchmarks.py)

## Repeat

Use the same power source, plan and comparable pages for both apps. Let each settle before measuring. Include the original GUI, tray and launcher service. For Minimalist's open Cooling page, include its existing `mcc.exe hardware observe-loop` child with `-ExternalProcessIds`.

```powershell
.\scripts\measure-app.ps1 -ProcessIds @(GUI_PID) -ExternalProcessIds @(HELPER_PID) -Label visible -Seconds 30 -WindowHandle WINDOW_HANDLE -Output reports/local/visible.json
```

Repeat twice while open. Hide the app and repeat without a window handle; for Minimalist, verify the observer exits and omit its PID. The script changes no hardware settings.
