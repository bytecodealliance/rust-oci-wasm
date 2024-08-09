use anyhow::Context;
use oci_client::{
    client::{ClientConfig, ClientProtocol},
    errors::OciDistributionError,
};
use oci_wasm::{
    Component, WasmClient, WasmConfig, COMPONENT_OS, WASM_ARCHITECTURE, WASM_LAYER_MEDIA_TYPE,
    WASM_MANIFEST_CONFIG_MEDIA_TYPE, WASM_MANIFEST_MEDIA_TYPE,
};

const DOCKER_CONTAINER_NAME: &str = "rust-oci-wasm-test";
const REGISTRY_URL: &str = "localhost:5001";

// NOTE(thomastaylor312): Tried using `Drop` on a struct to cleanup the registry, but I don't think
// it runs for statics. This just makes sure we only start the registry once
static ONCE: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn setup_registry() -> anyhow::Result<()> {
    match std::process::Command::new("docker")
        .args(["stop", DOCKER_CONTAINER_NAME])
        .status()
    {
        Ok(_) => {}
        Err(e) => {
            anyhow::bail!("Failed to run cleanup step: {}", e);
        }
    };
    let status = std::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "--name",
            DOCKER_CONTAINER_NAME,
            "-d",
            "-p",
            "5001:5000",
            "registry:2",
        ])
        .status()
        .context("Failed to start docker registry")?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to start docker registry"));
    }
    Ok(())
}

fn get_client() -> WasmClient {
    let client = oci_client::Client::new(ClientConfig {
        protocol: ClientProtocol::HttpsExcept(vec!["localhost:5001".to_string()]),
        // This makes sure for failure tests we always try to pull the linux image
        platform_resolver: Some(Box::new(|manifests| {
            manifests
                .iter()
                .find(|entry| {
                    entry.platform.as_ref().map_or(false, |platform| {
                        platform.os == "linux" && platform.architecture == "amd64"
                    })
                })
                .map(|entry| entry.digest.clone())
        })),
        ..Default::default()
    });
    WasmClient::new(client)
}

#[tokio::test]
async fn test_push_and_pull() {
    let _ = ONCE
        .get_or_try_init(setup_registry)
        .await
        .expect("Should be able to start docker registry");
    let client = get_client();

    let image = oci_client::Reference::try_from(format!("{REGISTRY_URL}/test/test:0.0.1")).unwrap();

    let (conf, component) = WasmConfig::from_component(
        "./tests/data/component.wasm",
        Some("Bugs Bunny".to_string()),
    )
    .await
    .expect("Should be able to parse component and create config");
    let resp = client
        .push(
            &image,
            &oci_client::secrets::RegistryAuth::Anonymous,
            component,
            conf,
            None,
        )
        .await
        .expect("Should be able to push component");
    assert!(
        !resp.config_url.is_empty(),
        "Should have a config url in the response"
    );
    assert!(
        !resp.manifest_url.is_empty(),
        "Should have a manifest url in the response"
    );

    // Check that just pulling the manifest works and check the config
    let (_, conf, _) = client
        .pull_manifest_and_config(&image, &oci_client::secrets::RegistryAuth::Anonymous)
        .await
        .expect("Should be able to pull manifest and config");
    assert_eq!(
        conf.author.expect("Author should be set"),
        "Bugs Bunny",
        "Should have the correct author set"
    );
    assert_eq!(
        conf.architecture, WASM_ARCHITECTURE,
        "Should have the correct architecture set in config"
    );
    assert_eq!(conf.os, COMPONENT_OS, "Should have the right OS value set");
    let component_info = conf
        .component
        .expect("Should have component information set in config");

    // To find these expected types, run wasm-tools component wit test/data/component.wasm from the
    // top level of the repo
    let expected_exports = vec!["wasi:http/incoming-handler@0.2.0".to_string()];
    // This is already sorted
    let expected_imports = vec![
        "wasi:cli/environment@0.2.0".to_string(),
        "wasi:cli/exit@0.2.0".to_string(),
        "wasi:cli/stderr@0.2.0".to_string(),
        "wasi:cli/stdin@0.2.0".to_string(),
        "wasi:cli/stdout@0.2.0".to_string(),
        "wasi:clocks/wall-clock@0.2.0".to_string(),
        "wasi:filesystem/preopens@0.2.0".to_string(),
        "wasi:filesystem/types@0.2.0".to_string(),
        "wasi:http/types@0.2.0".to_string(),
        "wasi:io/error@0.2.0".to_string(),
        "wasi:io/streams@0.2.0".to_string(),
    ];

    let exports = component_info.exports;
    let mut imports = component_info.imports;
    imports.sort();

    assert_eq!(
        exports, expected_exports,
        "Expected exports to match:\nGot: {exports:?}\nExpected:\n{expected_exports:?}"
    );
    assert_eq!(
        imports, expected_imports,
        "Expected imports to match:\nGot: {imports:?}\nExpected:\n{expected_imports:?}"
    );

    // Now try to pull and make all the data is correct
    let data = client
        .pull(&image, &oci_client::secrets::RegistryAuth::Anonymous)
        .await
        .expect("Should be able to pull component");
    assert_eq!(
        data.config.media_type, WASM_MANIFEST_CONFIG_MEDIA_TYPE,
        "Should have the proper config media type"
    );
    assert_eq!(data.layers.len(), 1, "Should have exactly one layer");
    assert_eq!(
        data.layers[0].media_type, WASM_LAYER_MEDIA_TYPE,
        "Should have the proper layer media type"
    );
    assert_eq!(
        data.manifest
            .expect("Should have manifest present")
            .media_type
            .expect("Must have media type"),
        WASM_MANIFEST_MEDIA_TYPE,
        "Should have the proper manifest media type"
    );

    // As a sanity check, make sure we can still parse the bytes out (by loading a component)
    let _ = Component::from_raw_component(&data.layers[0].data)
        .expect("Returned bytes should still be valid");
}

#[tokio::test]
async fn pulling_non_wasm_should_fail() {
    let _ = ONCE
        .get_or_try_init(setup_registry)
        .await
        .expect("Should be able to start docker registry");
    let client = get_client();
    // Using an older wasmcloud image because otherwise the pull doesn't work due to platform
    // mismatch on things like a Mac. I tried this with an alpine image first ghcr.io/wasmcloud/component-echo-messaging:0.1.0
    let image = oci_client::Reference::try_from("docker.io/library/alpine:3").unwrap();
    // ImageData doesn't implement debug so we can't use `expect_err` here
    let err = match client
        .pull(&image, &oci_client::secrets::RegistryAuth::Anonymous)
        .await
    {
        Ok(_) => panic!("Should not be able to pull non wasm component"),
        Err(e) => e,
    };
    assert!(
        matches!(
            err.downcast::<OciDistributionError>().unwrap(),
            OciDistributionError::IncompatibleLayerMediaTypeError(_)
        ),
        "Should have returned an incompatible layer media type error"
    );
}

#[tokio::test]
async fn test_binary_wit_parse() {
    let (conf, _) = WasmConfig::from_component("./tests/data/binary_wit.wasm", None)
        .await
        .expect("Should be able to parse binary wit");

    assert_eq!(
        conf.architecture, WASM_ARCHITECTURE,
        "Should have the correct architecture set in config"
    );
    assert_eq!(conf.os, COMPONENT_OS, "Should have the right OS value set");
    let component_info = conf
        .component
        .expect("Should have component information set in config");

    let mut expected_exports = vec![
        "wasi:http/incoming-handler@0.2.0".to_string(),
        "wasi:http/types@0.2.0".to_string(),
        "wasi:http/outgoing-handler@0.2.0".to_string(),
        "wasi:http/proxy@0.2.0".to_string(),
        "wasi:http/imports@0.2.0".to_string(),
    ];
    expected_exports.sort();

    let mut exports = component_info.exports;
    exports.sort();

    assert_eq!(
        exports, expected_exports,
        "Expected exports to match:\nGot: {exports:?}\nExpected:\n{expected_exports:?}"
    );
    assert!(component_info.imports.is_empty(), "Should have no imports");
}
