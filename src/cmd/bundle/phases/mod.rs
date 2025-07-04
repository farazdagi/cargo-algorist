pub mod complete_bundling;
pub mod expand_mods;
pub mod parse_binary;
pub mod traverse_crates;
pub mod utils;

/// Represents a phase in the bundling process.
pub trait BunlingPhase: Sized {}

pub use {
    complete_bundling::CompleteBundling,
    expand_mods::ExpandMods,
    parse_binary::ParseBinary,
    traverse_crates::TraverseCrates,
};
