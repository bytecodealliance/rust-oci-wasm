use jiff::Timestamp;
use oci_wasm::{ToConfig, WasmConfig, WASM_MANIFEST_CONFIG_MEDIA_TYPE};
use serde_json::json;

#[test]
fn config_timestamps_remain_rfc3339() {
    for (input, expected) in [
        ("2026-11-19T14:30:00Z", "2026-11-19T14:30:00Z"),
        ("2026-11-19T14:30:00.123Z", "2026-11-19T14:30:00.123Z"),
        ("2026-11-19T14:30:00.123456Z", "2026-11-19T14:30:00.123456Z"),
        (
            "2026-11-19T14:30:00.123456789Z",
            "2026-11-19T14:30:00.123456789Z",
        ),
        (
            "2026-11-19T09:30:00.123456789-05:00",
            "2026-11-19T14:30:00.123456789Z",
        ),
        (
            "1969-12-31T23:59:59.123456789Z",
            "1969-12-31T23:59:59.123456789Z",
        ),
    ] {
        let mut value = json!({
            "created": input,
            "author": "Bugs Bunny",
            "architecture": "wasm",
            "os": "wasip1",
            "layerDigests": ["sha256:fixture"],
            "component": null,
        });
        let config = WasmConfig::try_from(value.to_string()).unwrap();
        assert_eq!(config.created, expected.parse::<Timestamp>().unwrap());

        let config = config.to_config().unwrap();
        assert_eq!(config.media_type, WASM_MANIFEST_CONFIG_MEDIA_TYPE);
        value["created"] = json!(expected);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&config.data).unwrap(),
            value
        );
    }
}

#[test]
fn invalid_config_timestamps_are_rejected() {
    for created in [
        "not a timestamp",
        "2026-13-01T00:00:00Z",
        "2026-11-19T14:30:00",
    ] {
        let value = json!({
            "created": created,
            "architecture": "wasm",
            "os": "wasip1",
            "layerDigests": [],
        });
        assert!(
            WasmConfig::try_from(value.to_string()).is_err(),
            "{created}"
        );
    }
}

#[test]
fn generated_configs_use_current_timestamps() {
    let before = Timestamp::now();
    let (module, _) = WasmConfig::from_raw_module(b"\0asm\x01\0\0\0".to_vec(), None).unwrap();
    let (component, _) =
        WasmConfig::from_raw_component(include_bytes!("data/component.wasm").to_vec(), None)
            .unwrap();
    let after = Timestamp::now();

    for config in [module, component] {
        assert!(before <= config.created && config.created <= after);
        let raw = config.to_config().unwrap();
        let parsed = WasmConfig::try_from(raw.data.as_ref()).unwrap();
        assert_eq!(parsed.created, config.created);
    }
}
