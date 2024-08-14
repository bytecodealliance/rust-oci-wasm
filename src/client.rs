use std::{collections::BTreeMap, ops::Deref};

use oci_client::{
    client::{ImageData, ImageLayer, PushResponse},
    manifest::OciImageManifest,
    secrets::RegistryAuth,
    Client, Reference,
};

use crate::{
    config::ToConfig, WasmConfig, WASM_LAYER_MEDIA_TYPE, WASM_MANIFEST_CONFIG_MEDIA_TYPE,
    WASM_MANIFEST_MEDIA_TYPE,
};

/// A light wrapper around the oci-distribution client to add support for the `application/wasm` type
pub struct WasmClient {
    client: Client,
}

impl AsRef<Client> for WasmClient {
    fn as_ref(&self) -> &Client {
        &self.client
    }
}

impl Deref for WasmClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl From<Client> for WasmClient {
    fn from(value: Client) -> Self {
        Self { client: value }
    }
}

impl From<WasmClient> for Client {
    fn from(value: WasmClient) -> Self {
        value.client
    }
}

impl WasmClient {
    /// Create a new client
    pub fn new(client: Client) -> Self {
        Self::from(client)
    }

    /// A convenience wrapper around [`Client::pull`] that pulls a wasm component and errors if
    /// there are layers that aren't wasm
    pub async fn pull(&self, image: &Reference, auth: &RegistryAuth) -> anyhow::Result<ImageData> {
        let image_data = self
            .client
            .pull(image, auth, vec![WASM_LAYER_MEDIA_TYPE])
            .await?;
        if image_data.layers.len() != 1 {
            anyhow::bail!("Wasm components must have exactly one layer");
        }

        if image_data.config.media_type != WASM_MANIFEST_CONFIG_MEDIA_TYPE {
            anyhow::bail!(
                "Wasm components must have a config of type {}",
                WASM_MANIFEST_CONFIG_MEDIA_TYPE
            );
        }

        Ok(image_data)
    }

    /// A convenience wrapper around [`Client::pull_manifest_and_config`] that parses the config as
    /// a [`WasmConfig`] type
    pub async fn pull_manifest_and_config(
        &self,
        image: &Reference,
        auth: &RegistryAuth,
    ) -> anyhow::Result<(OciImageManifest, WasmConfig, String)> {
        let (manifest, digest, config) = self.client.pull_manifest_and_config(image, auth).await?;
        if manifest.layers.len() != 1 {
            anyhow::bail!("Wasm components must have exactly one layer");
        }
        if manifest.media_type.as_deref().unwrap_or_default() != WASM_MANIFEST_MEDIA_TYPE {
            anyhow::bail!(
                "Wasm components must have a manifest of type {}",
                WASM_MANIFEST_MEDIA_TYPE
            );
        }

        if manifest.config.media_type != WASM_MANIFEST_CONFIG_MEDIA_TYPE {
            anyhow::bail!(
                "Wasm components must have a config of type {}",
                WASM_MANIFEST_CONFIG_MEDIA_TYPE
            );
        }

        let config = WasmConfig::try_from(config)?;
        Ok((manifest, config, digest))
    }

    /// A convenience wrapper around [`Client::push`] that pushes a wasm component or module with
    /// the given config and optional annotations for the manifest
    pub async fn push(
        &self,
        image: &Reference,
        auth: &RegistryAuth,
        component_layer: ImageLayer,
        config: impl ToConfig,
        annotations: Option<BTreeMap<String, String>>,
    ) -> anyhow::Result<PushResponse> {
        let layers = vec![component_layer];
        let config = config.to_config()?;
        let mut manifest = OciImageManifest::build(&layers, &config, annotations);
        manifest.media_type = Some(WASM_MANIFEST_MEDIA_TYPE.to_string());
        self.client
            .push(image, &layers, config, auth, Some(manifest))
            .await
            .map_err(Into::into)
    }
}
