use serde::{Deserialize, Serialize};
use std::{process::Command, time::Duration};

pub const SCRIPT: &str = include_str!("schema.ps1");
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub cim_type: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<Parameter>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub name: String,
    pub available: bool,
    pub methods: Vec<Method>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub namespace: String,
    pub classes: Vec<Class>,
}

#[derive(Debug, Serialize)]
pub struct Discovery {
    pub schema_version: u32,
    pub mode: &'static str,
    pub hardware_control_available: bool,
    pub metadata: Schema,
    pub note: &'static str,
}

pub fn parse(bytes: &[u8]) -> Result<Schema, String> {
    if bytes.len() > crate::process::OUTPUT_LIMIT {
        return Err("Schema output exceeds limit".into());
    }
    let data: Schema = serde_json::from_slice(bytes).map_err(|_| "Invalid schema output")?;
    if data.namespace != "root/WMI" || data.classes.len() != 2 {
        return Err("Unexpected schema scope".into());
    }
    for (class, expected) in data
        .classes
        .iter()
        .zip(["AcpiTest_MULong", "AcpiTest_EventULong"])
    {
        if class.name != expected
            || class.methods.len() > 32
            || (!class.available && !class.methods.is_empty())
            || (class.available && class.error.is_some())
        {
            return Err("Invalid class metadata".into());
        }
        if class
            .error
            .as_deref()
            .is_some_and(|e| e != "unavailable_or_access_denied")
        {
            return Err("Unexpected error details".into());
        }
        for method in &class.methods {
            if !identifier(&method.name)
                || !identifier(&method.return_type)
                || method.parameters.len() > 32
            {
                return Err("Invalid method metadata".into());
            }
            for p in &method.parameters {
                if !identifier(&p.name) || !identifier(&p.cim_type) {
                    return Err("Invalid parameter metadata".into());
                }
            }
        }
    }
    Ok(data)
}
fn identifier(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub fn discover() -> Result<Discovery, String> {
    let path = crate::platform::powershell_path()?;
    let mut command = Command::new(path);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        SCRIPT,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let bytes = crate::process::run(&mut command, Duration::from_secs(10))
        .map_err(|e| format!("Metadata discovery failed: {e:?}"))?;
    Ok(Discovery {
        schema_version: 1,
        mode: crate::MODE,
        hardware_control_available: false,
        metadata: parse(&bytes)?,
        note: "Schema presence is not evidence of safe register addresses, sensor units, fan support or recovery behavior. No vendor method was invoked.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        br#"{"namespace":"root/WMI","classes":[{"name":"AcpiTest_MULong","available":true,"methods":[{"name":"GetSetULong","return_type":"UInt32","parameters":[{"name":"Data","cim_type":"UInt64"}]}],"error":null},{"name":"AcpiTest_EventULong","available":false,"methods":[],"error":"unavailable_or_access_denied"}]}"#.to_vec()
    }
    #[test]
    fn parses_metadata_without_granting_control() {
        let s = parse(&fixture()).unwrap();
        assert_eq!(s.classes[0].methods[0].parameters[0].cim_type, "UInt64");
    }
    #[test]
    fn malformed_and_large_outputs_are_rejected() {
        assert!(parse(b"not JSON").is_err());
        assert!(parse(&vec![b' '; crate::process::OUTPUT_LIMIT + 1]).is_err());
    }
    #[test]
    fn wrong_namespace_and_class_are_rejected() {
        for (old, new) in [
            ("root/WMI", "root/other"),
            ("AcpiTest_MULong", "Other_Class"),
        ] {
            let f = String::from_utf8(fixture()).unwrap().replace(old, new);
            assert!(parse(f.as_bytes()).is_err());
        }
    }
    #[test]
    fn control_sequences_in_metadata_are_rejected() {
        let f = String::from_utf8(fixture())
            .unwrap()
            .replace("GetSetULong", "Get\\u001bMethod");
        assert!(parse(f.as_bytes()).is_err());
    }
    #[test]
    fn unknown_fields_are_rejected() {
        let f = String::from_utf8(fixture())
            .unwrap()
            .replace("\"namespace\":", "\"serial\":\"private\",\"namespace\":");
        assert!(parse(f.as_bytes()).is_err());
    }
    #[test]
    fn script_has_only_metadata_queries() {
        assert!(SCRIPT.contains("Get-CimClass"));
        for forbidden in [
            "Invoke-CimMethod",
            "Invoke-WmiMethod",
            "Set-CimInstance",
            "Get-CimInstance",
            "ManagementObject",
            "GetSetULong(",
        ] {
            assert!(!SCRIPT.contains(forbidden));
        }
    }
}
