//! Server-independent game domains.
//!
//! Player mutations belong to a player-scoped manager (for example
//! `hero::HeroManager`); handlers decode requests and call that owner. Generated
//! config tables stay data-only, with reusable lookups implemented on
//! `config::GameDB`.

pub mod activity;
pub mod arcade;
pub mod battle_setup;
pub mod bp;
pub mod charge;
pub mod collection;
pub mod command_post;
pub mod critter;
pub mod dungeon;
pub mod error;
pub mod exploration;
pub mod explore_engine;
pub mod explore_chest;
pub mod explore_entry;
pub mod explore_move;
pub mod explore_rune;
pub mod explore_battle;
pub mod explore_exit;
pub mod explore_teleport;
pub mod explore_rock;
pub mod explore_spike;
pub mod explore_reset;
pub mod explore_light;
pub mod explore_pipe;
pub mod explore_bonus;
pub mod fairyland;
pub mod guide;
pub mod hero;
pub mod hero_invitation;
pub mod inventory;
pub mod investigate;
pub mod mail;
pub mod odyssey;
pub mod preferences;
pub mod profile;
pub mod red_dot;
pub mod reward;
pub mod room;
pub mod rouge;
pub mod sign_in;
pub mod social;
pub mod stat;
pub mod store;
pub mod story;
pub mod summon;
pub mod survival;
pub mod task;
pub mod teaching;
pub mod time;
pub mod turnback;
pub mod types;
pub mod udimo;

pub use error::LogicError;

#[cfg(test)]
mod explore_route_tests;

#[cfg(test)]
pub(crate) fn init_test_server_config() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| common::init_config(common::config::ServerConfig::template()));
}
