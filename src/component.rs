use std::path::Path;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use wit_parser::{Resolve, WorldId, WorldKey};

/// Information about the component in the manifest. This is generally synthesized from a
/// component's world
#[derive(Serialize, Deserialize, Debug)]
pub struct Component {
    /// A list of all exports from the component
    pub exports: Vec<String>,
    /// A list of all imports from the component
    pub imports: Vec<String>,
    // This is optional metadata for indexing. Implementations MAY use this information to fetch
    // other data to inspect the specified world
    pub target: Option<String>,
}

impl Component {
    /// Create a component from a parsed [`Resolve`] and [`WorldId`]. This is a lower level function
    /// for when you've already parsed a resolve and have the world ID
    ///
    /// Returns an error only if the world doesn't exist in the resolve.
    pub fn from_world(resolve: &Resolve, world_id: WorldId) -> anyhow::Result<Self> {
        let world = resolve
            .worlds
            .iter()
            .find_map(|(id, w)| (id == world_id).then_some(w))
            .context("component world not found")?;
        Ok(Component {
            exports: world
                .exports
                .keys()
                .filter_map(|key| name_from_key_and_item(resolve, key))
                .collect(),
            imports: world
                .imports
                .keys()
                .filter_map(|key| name_from_key_and_item(resolve, key))
                .collect(),
            target: None,
        })
    }

    /// Create a component by loading the given component from the filesystem
    pub async fn from_component(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let data = tokio::fs::read(path).await.context("Unable to read file")?;
        Self::from_raw_component(data)
    }

    /// Create a component from the raw bytes of the component
    pub fn from_raw_component(raw: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        let (resolve, world) = match wit_component::decode(raw.as_ref())
            .context("failed to decode WIT component")?
        {
            wit_component::DecodedWasm::Component(resolve, world) => (resolve, world),
            wit_component::DecodedWasm::WitPackage(..) => {
                bail!("Found a binary wit package, not a component. Please use the from_wit_package or from_raw_wit_package functions")
            }
        };

        Self::from_world(&resolve, world)
    }

    /// Create a component from the raw bytes of a binary wit package component
    pub async fn from_wit_package(
        path: impl AsRef<Path>,
        world_name: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let data = tokio::fs::read(path).await.context("Unable to read file")?;
        Self::from_raw_wit_package(data, world_name)
    }

    /// Create a component from the raw bytes of a binary wit package component
    pub fn from_raw_wit_package(
        raw: impl AsRef<[u8]>,
        world_name: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let resolve = match wit_component::decode(raw.as_ref())
            .context("failed to decode WIT component")?
        {
            wit_component::DecodedWasm::WitPackage(resolve, _) => resolve,
            wit_component::DecodedWasm::Component(..) => {
                bail!("Found a component, not a binary wit package. Please use the from_component or from_raw_component functions")
            }
        };

        let world = resolve
            .worlds
            .iter()
            .find_map(|(id, w)| (w.name == world_name.as_ref()).then_some(id))
            .context(format!(
                "Unable to find matching world for name {}",
                world_name.as_ref()
            ))?;

        Self::from_world(&resolve, world)
    }
}

fn name_from_key_and_item(resolve: &Resolve, key: &WorldKey) -> Option<String> {
    match key {
        WorldKey::Interface(id) => {
            // This function returns the full name of the interface
            resolve.id_of(*id)
        }
        WorldKey::Name(name) => Some(name.to_owned()),
    }
}
