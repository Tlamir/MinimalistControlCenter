//! Shareable support information without vendor methods, EC access or machine identifiers.
use crate::{Reading, control, platform};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Diagnostics {
    pub schema_version: u32,
    pub version: &'static str,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub mode: &'static str,
    pub hardware_writes: u32,
    pub identity: platform::Identity,
    pub supported_fingerprint: bool,
    pub ec_firmware_version: Reading<String>,
    pub notes: [&'static str; 3],
}

pub fn collect() -> Diagnostics {
    from_identity(platform::identity())
}

fn from_identity(identity: platform::Identity) -> Diagnostics {
    let supported_fingerprint = control::supported_identity(
        identity.manufacturer.value.as_deref().unwrap_or(""),
        identity.model.value.as_deref().unwrap_or(""),
        identity.bios_version.value.as_deref().unwrap_or(""),
        identity.cpu.value.as_deref().unwrap_or(""),
        &identity.display_adapters,
    );
    Diagnostics {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        operating_system: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        mode: "read_only",
        hardware_writes: 0,
        identity,
        supported_fingerprint,
        ec_firmware_version: Reading::unavailable(
            "No verified read-only EC firmware version query; no EC probe was performed",
        ),
        notes: [
            "Fingerprint match alone does not authorize controls; ownership, capabilities and administrator checks still apply.",
            "No vendor methods, EC transactions, startup changes or elevation are requested by this report.",
            "Review before sharing. This report omits serial numbers, usernames, hostnames, MAC addresses and file paths.",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> platform::Identity {
        platform::Identity {
            manufacturer: Reading::available("MONSTER".into()),
            model: Reading::available("TULPAR T5 V17.1".into()),
            bios_version: Reading::available("N.1.06".into()),
            cpu: Reading::available("Intel Core i7-8750H".into()),
            display_adapters: vec!["NVIDIA GeForce GTX 1060".into()],
        }
    }

    #[test]
    fn report_distinguishes_fingerprint_from_unverified_ec_version() {
        let report = from_identity(identity());
        assert!(report.supported_fingerprint);
        assert_eq!(report.hardware_writes, 0);
        assert!(report.ec_firmware_version.value.is_none());
        let mut other = identity();
        other.bios_version = Reading::available("different BIOS".into());
        assert!(!from_identity(other).supported_fingerprint);
    }

    #[test]
    fn missing_identity_is_unknown_and_never_matches() {
        let mut missing = identity();
        missing.model = Reading::unavailable("Not available");
        let report = serde_json::to_value(from_identity(missing)).unwrap();
        assert_eq!(report["supported_fingerprint"], false);
        assert!(report["identity"]["model"]["value"].is_null());
        assert!(report["identity"]["model"]["unavailable_reason"].is_string());
    }
}
