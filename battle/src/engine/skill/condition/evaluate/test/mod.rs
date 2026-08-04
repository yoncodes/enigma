use super::*;
use crate::engine::skill::condition::parse::BuffAddedScope;
use crate::test_support::init_config;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

mod actions;
mod buffs;
mod counts;
mod mechanics;
mod targets;

fn exact_condition(opcode: i32, type_name: &str, args: &[&str]) -> ParsedCondition {
    let raw_args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let kind = crate::engine::skill::condition::registry::parse(opcode, type_name, &raw_args)
        .expect("exact condition is registered");
    ParsedCondition {
        opcode,
        type_name: type_name.to_owned(),
        kind,
        raw_args,
    }
}
