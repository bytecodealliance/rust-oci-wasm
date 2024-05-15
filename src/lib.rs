mod client;
mod component;
mod config;

pub use client::WasmClient;
pub use component::Component;
pub use config::{ToConfig, WasmConfig};

pub const WASM_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
pub const WASM_MANIFEST_CONFIG_MEDIA_TYPE: &str = "application/vnd.wasm.config.v0+json";
pub const WASM_LAYER_MEDIA_TYPE: &str = "application/wasm";
pub const WASM_ARCHITECTURE: &str = "wasm";
pub const MODULE_OS: &str = "wasip1";
pub const COMPONENT_OS: &str = "wasip2";
