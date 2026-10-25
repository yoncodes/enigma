use super::*;
use crate::engine::manager::conduit::ConduitCounterKind;
use crate::engine::skill::condition::{
    ConditionCompare, buff::BuffConditionMode, none::NoneMode, parse::BuffAddedScope,
};

mod mane;

mod event_routes;
mod predicates;
mod setup_routes;
mod target_routes;
