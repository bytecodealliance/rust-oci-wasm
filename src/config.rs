use std::collections::BTreeMap;

use anyhow::Context;
use chrono::{DateTime, Utc};
use oci_client::client::{Config, ImageLayer};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::{
    Component, COMPONENT_OS, MODULE_OS, WASM_ARCHITECTURE, WASM_LAYER_MEDIA_TYPE,
    WASM_MANIFEST_CONFIG_MEDIA_TYPE,
};

// A convenience trait that indicates a type can be converted into an OCI manifest config
pub trait ToConfig {
    /// Convert the type into an OCI manifest config
    fn to_config(&self) -> anyhow::Result<Config>;
}

/// The config type struct for `application/wasm`
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WasmConfig {
    /// The time when the config was created.
    pub created: DateTime<Utc>,
    /// The optional name of the author of the config.
    pub author: Option<String>,
    /// The architecture of the artifact. This is always `wasm`.
    pub architecture: String,
    /// The OS name of the artifact. Possible options: wasip1, wasip2. For plain wasm, this should
    /// be wasip1 as this must match a GOOS value and it doesn’t have one for plain Wasm
    ///
    /// Eventually this will go away when we hit a 1.0 but we need it for now
    pub os: String,
    /// This field contains a list of digests of each of the layers from the manifest in the same
    /// order as they are listed in the manfiest. This exists because we need to have a unique list
    /// here so that the hash of the config (used as the ID) is unique every time
    /// (https://github.com/opencontainers/image-spec/pull/1173)
    pub layer_digests: Vec<String>,
    /// Information about the component in the manifest. This is required when the `os` field is
    /// `wasip2`
    pub component: Option<Component>,
}

pub struct AnnotatedWasmConfig<'a> {
    pub config: &'a WasmConfig,
    pub annotations: BTreeMap<String, String>,
}

impl WasmConfig {
    /// A helper for loading a component from a file and returning the proper config and
    /// [`ImageLayer`]. The returned config will have the created time set to now and all other
    /// fields set for a component.
    pub async fn from_component(
        path: impl AsRef<std::path::Path>,
        author: Option<String>,
    ) -> anyhow::Result<(Self, ImageLayer)> {
        let raw = tokio::fs::read(path).await.context("Unable to read file")?;
        Self::from_raw_component(raw, author)
    }

    /// Same as [`WasmConfig::from_component`] but for raw component bytes
    pub fn from_raw_component(
        raw: Vec<u8>,
        author: Option<String>,
    ) -> anyhow::Result<(Self, ImageLayer)> {
        let component = Component::from_raw_component(&raw)?;
        let config = Self {
            created: Utc::now(),
            author,
            architecture: WASM_ARCHITECTURE.to_string(),
            os: COMPONENT_OS.to_string(),
            layer_digests: vec![sha256_digest(&raw)],
            component: Some(component),
        };
        Ok((
            config,
            ImageLayer {
                data: raw.into(),
                media_type: WASM_LAYER_MEDIA_TYPE.to_string(),
                annotations: None,
            },
        ))
    }

    /// A helper for loading a plain wasm module and returning the proper config and [`ImageLayer`].
    /// The returned config will have the created time set to now and all other fields set for a
    /// plain wasm module.
    pub async fn from_module(
        path: impl AsRef<std::path::Path>,
        author: Option<String>,
    ) -> anyhow::Result<(Self, ImageLayer)> {
        let raw = tokio::fs::read(path).await.context("Unable to read file")?;
        Self::from_raw_module(raw, author)
    }

    /// Same as [`WasmConfig::from_module`] but for raw module bytes
    pub fn from_raw_module(
        raw: Vec<u8>,
        author: Option<String>,
    ) -> anyhow::Result<(Self, ImageLayer)> {
        let config = Self {
            created: Utc::now(),
            author,
            architecture: WASM_ARCHITECTURE.to_string(),
            os: MODULE_OS.to_string(),
            layer_digests: vec![sha256_digest(&raw)],
            component: None,
        };
        Ok((
            config,
            ImageLayer {
                data: raw.into(),
                media_type: WASM_LAYER_MEDIA_TYPE.to_string(),
                annotations: None,
            },
        ))
    }

    /// Adds annotations to this [`WasmConfig`].
    #[must_use]
    pub fn with_annotations(
        &'_ self,
        annotations: BTreeMap<String, String>,
    ) -> AnnotatedWasmConfig<'_> {
        AnnotatedWasmConfig {
            config: self,
            annotations,
        }
    }
}

impl ToConfig for AnnotatedWasmConfig<'_> {
    /// Generate a [`Config`] for this [`WasmConfig`]
    fn to_config(&self) -> anyhow::Result<Config> {
        let mut config = self.config.to_config()?;
        config.annotations = Some(self.annotations.clone());
        Ok(config)
    }
}

impl ToConfig for WasmConfig {
    /// Generate a [`Config`] for this [`WasmConfig`]
    fn to_config(&self) -> anyhow::Result<Config> {
        serde_json::to_vec(self)
            .map(|data| Config {
                data: data.into(),
                media_type: WASM_MANIFEST_CONFIG_MEDIA_TYPE.to_string(),
                annotations: None,
            })
            .map_err(Into::into)
    }
}

// NOTE: There are a bunch of implementations here because we can't do a generic implementation
// across T for AsRef<[u8]>

impl TryFrom<String> for WasmConfig {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(&value).map_err(Into::into)
    }
}

impl TryFrom<Vec<u8>> for WasmConfig {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(Into::into)
    }
}

impl TryFrom<&str> for WasmConfig {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(Into::into)
    }
}

impl TryFrom<&[u8]> for WasmConfig {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        serde_json::from_slice(value).map_err(Into::into)
    }
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", sha2::Sha256::digest(bytes))
}
