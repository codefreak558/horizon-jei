//! JEI for Horizon: an item and recipe viewer, a port of Just Enough Items (mezz, MIT).
pub mod kit_collection;
pub mod kit_math;
pub mod runtime;

pub use runtime::*;

#[cfg(test)]
mod tests;

#[derive(Default)]
struct Jei;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Jei {}
