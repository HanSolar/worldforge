//! WorldForge as a PhotoCraft plug-in.
//!
//! Thin adapter: [`abi`] implements the host's WebAssembly ABI v1 exports and
//! [`manifest`] holds the manifest. All generation lives in `worldforge-core`,
//! which knows nothing about PhotoCraft (spec §3.1).
//!
//! Build the module with:
//!
//! ```text
//! rustup target add wasm32-unknown-unknown
//! cargo build -p worldforge-photocraft --release --target wasm32-unknown-unknown
//! ```
//!
//! The artefact is `target/wasm32-unknown-unknown/release/worldforge_photocraft.wasm`.

pub mod abi;
pub mod manifest;

pub use abi::{ABI_VERSION, pc_abi_version, pc_alloc, pc_filter, pc_manifest};
pub use manifest::{MANIFEST, PLUGIN_ID, PLUGIN_NAME, PLUGIN_VERSION, manifest_json};

/// Re-exported so the CLI and tests can generate without depending on core
/// directly.
pub use worldforge_core as core;
