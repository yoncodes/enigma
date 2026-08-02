use crate::engine::{
    event::kind::EventKind,
    skill::condition::{
        ParsedCondition,
        parse::{ParsedConditionKind, first_i32, parse_fixed, parse_i32},
        query,
    },
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
    let [max_count, _unknown] = parse_fixed(args)?;
    Some(ParsedConditionKind::TeamEntityExited { max_count })
}

pub fn team_entity_exit_limit(conditions: &[ParsedCondition]) -> Option<i32> {
    query::find(conditions, &|condition| {
        matches!(condition.kind, ParsedConditionKind::TeamEntityExited { .. })
    })
    .and_then(|condition| match condition.kind {
        ParsedConditionKind::TeamEntityExited { max_count } if max_count > 0 => Some(max_count),
        _ => None,
    })
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
