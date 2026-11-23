use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector, BuffSetState},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{
        action::{SkillInvocation, SkillRequest},
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::DuduBoneContinueChannel) {
        return None;
    }
    if !matches!(event, BattleEvent::Kind(EventKind::SmallRoundEnd)) {
        return Some(Vec::new());
    }
    let [
        bane_buff_id,
        _compound_cap,
        first_stacks,
        ending_skill,
        _target_code,
    ] = subscriber.args.as_slice()
    else {
        return None;
    };
    if *bane_buff_id <= 0 || *first_stacks <= 0 || *ending_skill <= 0 {
        return None;
    }

    let origin = super::command_origin(subscriber)?;
    let completed_rounds = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?
        .ex_info
        .unwrap_or_default()
        .max(0);
    let available = managers.ex_point.get(subscriber.owner_uid).max(0);
    let requested = if completed_rounds == 0 {
        0
    } else {
        2_i32.saturating_add(completed_rounds.saturating_mul(2))
    };
    let spent = available.min(requested);
    let ending = requested > available;
    let stacks = if completed_rounds == 0 {
        *first_stacks
    } else {
        spent / 2
    };

    let mut ops = Vec::new();
    if spent > 0 {
        ops.push(RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: subscriber.owner_uid,
                delta: -spent,
                config_effect: 0,
                effect_type: EffectType::Expointchange as i32,
            }),
        )));
    }
    if stacks > 0 {
        ops.extend(
            pool.enemies(subscriber.owner_uid, false)
                .iter()
                .filter(|enemy| managers.hp.current(enemy.uid) > 0)
                .map(|enemy| {
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                        origin,
                        source_uid: subscriber.owner_uid,
                        target_uid: enemy.uid,
                        buff_id: *bane_buff_id,
                        amount: Some(stacks),
                        occurrences: 1,
                        child_uid_reservations: 0,
                    })))
                }),
        );
    }
    if ending {
        let invocation: SkillInvocation = SkillRequest {
            source_uid: subscriber.owner_uid,
            skill_id: *ending_skill,
        }
        .into();
        ops.push(RuleOp::Skill(invocation));
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
            },
        ))));
    } else {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: Some(completed_rounds.saturating_add(1)),
                params: None,
                act_info: None,
            },
        ))));
    }
    Some(ops)
}

pub fn compound_offset(managers: &BattleManagers, source_uid: i64) -> i32 {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == source_uid)
        .filter(|feature| super::is_kind(feature, BuffActKind::DuduBoneContinueChannel))
        .filter_map(|feature| {
            let [act_id, _bane_buff_id, cap, ..] = feature.values.as_slice() else {
                return None;
            };
            let _ = act_id;
            Some(
                managers
                    .buff
                    .snapshot(feature.owner_uid, feature.buff_uid)?
                    .ex_info
                    .unwrap_or_default()
                    .min(*cap)
                    .max(0),
            )
        })
        .max()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
