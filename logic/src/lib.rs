//! Server-independent game domains.
//!
//! Player mutations belong to a player-scoped manager (for example
//! `hero::HeroManager`); handlers decode requests and call that owner. Generated
//! config tables stay data-only, with reusable lookups implemented on
//! `config::GameDB`.

pub mod activity;
pub mod battle_setup;
pub mod bp;
pub mod charge;
pub mod collection;
pub mod command_post;
pub mod critter;
pub mod dungeon;
pub mod error;
pub mod exploration;
pub mod fairyland;
pub mod guide;
pub mod hero;
pub mod inventory;
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
pub mod time;
pub mod turnback;
pub mod types;
pub mod udimo;

pub use error::LogicError;
