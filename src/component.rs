use std::{collections::HashSet, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use wit_parser::{PackageId, Resolve, WorldId};

/// Information about the component in the manifest. This is generally synthesized from a
/// component's world
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
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
                .map(|key| resolve.name_world_key(key))
                .collect(),
            imports: world
                .imports
                .keys()
                .map(|key| resolve.name_world_key(key))
                .collect(),
            target: None,
        })
    }

    /// Create a component from a parsed [`Resolve`] and [`PackageId`]. This is a lower level
    /// function for when you have already parsed a binary wit package and have the package ID and
    /// resolve available. This outputs a component with all exports and an empty imports list.
    ///
    /// Returns an error only if the package doesn't exist in the resolve.
    pub fn from_package(resolve: &Resolve, pkg_id: PackageId) -> anyhow::Result<Self> {
        let pkg = resolve.packages.get(pkg_id).context("package not found")?;
        let mut exports = pkg
            .worlds
            .iter()
            .filter_map(|(_name, world_id)| {
                let world = resolve.worlds.get(*world_id)?;
                let mut exports = world
                    .exports
                    .keys()
                    .map(|key| resolve.name_world_key(key))
                    .collect::<Vec<_>>();
                let mut fully_qualified_world =
                    format!("{}:{}/{}", pkg.name.namespace, pkg.name.name, world.name);
                if let Some(ver) = pkg.name.version.as_ref() {
                    fully_qualified_world.push('@');
                    fully_qualified_world.push_str(&ver.to_string());
                }
                exports.push(fully_qualified_world);
                Some(exports)
            })
            .flatten()
            .collect::<HashSet<_>>();
        exports.extend(pkg.interfaces.values().filter_map(|id| resolve.id_of(*id)));
        Ok(Component {
            exports: exports.into_iter().collect(),
            imports: vec![],
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
        match wit_component::decode(raw.as_ref()).context("failed to decode WIT component")? {
            wit_component::DecodedWasm::Component(resolve, world) => {
                Self::from_world(&resolve, world)
            }
            wit_component::DecodedWasm::WitPackage(resolve, pkg_id) => {
                Self::from_package(&resolve, pkg_id)
            }
        }
    }
}
