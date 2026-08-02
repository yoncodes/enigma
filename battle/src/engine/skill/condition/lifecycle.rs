use crate::engine::{
    event::kind::EventKind,
    skill::condition::parse::{ParsedConditionKind, first_i32, parse_i32},
};

pub fn enter_fight(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::Lifecycle(LifecycleMode::EnterFight))
}

pub fn battle_start(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::Lifecycle(LifecycleMode::BattleStart))
}

pub fn round_interval(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::RoundInterval {
        start_round: first_i32(args)?,
        period: args.get(1).and_then(|arg| parse_i32(arg)).unwrap_or(0),
    })
}

pub fn after_round(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    (args.len() == 1).then_some(())?;
    Some(ParsedConditionKind::RoundInterval {
        start_round: first_i32(args)?,
        period: 1,
    })
}

pub fn entity_dead(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::EntityDead)
}

pub fn team_entity_exited(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    (args.len() == 2).then_some(ParsedConditionKind::TeamEntityExited)
}

pub fn period_then_start(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    if args.len() != 2 {
        return None;
    }

    Some(ParsedConditionKind::RoundInterval {
        period: first_i32(args)?,
        start_round: parse_i32(args.get(1)?)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleMode {
    BattleStart,
    EnterFight,
    Unconditional,
    RoundStart,
    CardSetup,
    BeforeApResolve,
    AfterRoundStart,
    None,
}

pub fn event_for_lifecycle(mode: LifecycleMode) -> Option<EventKind> {
    match mode {
        LifecycleMode::BattleStart | LifecycleMode::EnterFight => Some(EventKind::EnterFight),
        LifecycleMode::RoundStart => Some(EventKind::RoundStart),
        LifecycleMode::AfterRoundStart => Some(EventKind::AfterRoundStart),
        _ => None,
    }
}
