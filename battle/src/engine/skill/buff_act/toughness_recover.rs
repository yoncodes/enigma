use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        toughness::ToughnessRecover,
    },
    skill::{
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
    },
};

pub(super) fn on_break(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::ToughnessBroken { target_uid, .. } = event else {
        return None;
    };
    if *target_uid != subscriber.owner_uid || !subscriber.owner_alive {
        return Some(Vec::new());
    }

    update_timer(
        managers,
        *target_uid,
        subscriber.buff_uid,
        super::command_origin(subscriber)?,
        TimerStep::Break,
    )
}

/// Live (Version 7) re-sends the timer at every defender round start: `1, 0 + recover` across the two
/// starts after a break, and `1 - n` on the n-th defender start while unbroken (extra rounds count).
/// Without a defender start count nothing is sent while unbroken: live sends none at the opening, and
/// older layouts are unproven.
pub(super) fn at_round_start(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    defender_start_round: Option<i32>,
) -> Option<Vec<RuleOp>> {
    let [_, config_effect] = feature.values.as_slice() else {
        return None;
    };
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Toughness) {
        eprintln!(
            "toughness recover round start owner={} buff={} alive={} broken={} defender_start={:?} timer={:?}",
            feature.owner_uid,
            feature.buff_uid,
            feature.owner_alive,
            managers.toughness.is_broken(feature.owner_uid),
            defender_start_round,
            managers
                .buff
                .snapshot(feature.owner_uid, feature.buff_uid)
                .map(|buff| buff.act_info),
        );
    }
    if !feature.owner_alive {
        return Some(Vec::new());
    }
    let step = if managers.toughness.is_broken(feature.owner_uid) {
        TimerStep::BrokenStart(*config_effect)
    } else if let Some(start) = defender_start_round {
        TimerStep::UnbrokenStart(start)
    } else {
        return Some(Vec::new());
    };

    update_timer(
        managers,
        feature.owner_uid,
        feature.buff_uid,
        super::feature_command_origin(feature)?,
        step,
    )
}

#[derive(Clone, Copy)]
enum TimerStep {
    Break,
    BrokenStart(i32),
    UnbrokenStart(i32),
}

fn update_timer(
    managers: &BattleManagers,
    owner_uid: i64,
    buff_uid: i64,
    origin: CommandOrigin,
    step: TimerStep,
) -> Option<Vec<RuleOp>> {
    let act_id = origin.key.opcode;
    let mut info = managers.buff.snapshot(owner_uid, buff_uid)?.act_info;
    // Live grants the buff without the timer row and adds it with the first update.
    if !info.iter().any(|info| info.act_id == Some(act_id)) {
        info.push(sonettobuf::BuffActInfo {
            act_id: Some(act_id),
            param: Vec::new(),
            str_param: Some(String::new()),
        });
    }
    let timer = info.iter_mut().find(|info| info.act_id == Some(act_id))?;
    let (next, recover) = match (step, timer.param.as_slice()) {
        (TimerStep::Break, _) => (0, None),
        (TimerStep::BrokenStart(_), [0]) => (1, None),
        (TimerStep::BrokenStart(config_effect), [1]) => (0, Some(config_effect)),
        (TimerStep::UnbrokenStart(start), _) => (1 - start, None),
        _ => return None,
    };

    timer.param = vec![next];
    let mut ops = vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                origin,
                target_uid: owner_uid,
                buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(info),
            },
        ))),
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: owner_uid,
            buff_uid,
            act_id,
            params: vec![next],
            str_param: Some(String::new()),
            team_type: 0,
        }),
    ];
    if let Some(config_effect) = recover {
        ops.push(RuleOp::Command(BattleCommand::ToughnessRecover(
            ToughnessRecover {
                origin,
                target_uid: owner_uid,
                config_effect,
            },
        )));
    }
    Some(ops)
}
