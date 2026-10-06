pub mod catalog;
pub mod dungeon;
pub mod hero;
pub mod tooling;
pub mod tower;

mod battle;
// Internal families keep public visibility for sibling tests and explicit tooling exports.
#[allow(dead_code, unused_imports)]
mod engine;

pub use battle::Battle;
pub use engine::runtime::BattleOutcome;

#[cfg(test)]
pub(crate) mod test_support;
