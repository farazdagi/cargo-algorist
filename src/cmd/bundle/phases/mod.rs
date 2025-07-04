pub mod complete_bundling;
pub mod traverse_crates;
pub mod utils;

use std::path::PathBuf;

/// Represents a phase in the bundling process.
pub trait BunlingPhase: Sized {}

pub use {complete_bundling::CompleteBundling, traverse_crates::TraverseCrates};

/// Extract all used modules from the binary file.
pub struct ProcessBinaryFile {}

/// Find list of crates in the project, and for each crate invoke
/// `ProcessLibraryFile` stage.
pub struct CollectLibraryFiles {}

/// Recursively process a library file, expanding all used modules.
pub struct ProcessLibraryFile {
    pub crate_name: String,
    pub path: PathBuf,
    pub import_path: String,
}

impl BunlingPhase for ProcessBinaryFile {}
impl BunlingPhase for CollectLibraryFiles {}
impl BunlingPhase for ProcessLibraryFile {}
