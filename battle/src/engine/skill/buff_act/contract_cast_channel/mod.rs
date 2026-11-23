use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        condition::extra::skill_kind_from_is_extra,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [1, rate, lock_buff_id, owner_skill_id]
        if *rate > 0 && *lock_buff_id > 0 && *owner_skill_id > 0)
}

pub fn referenced_buff(args: &[i32]) -> Option<i32> {
    supports(args).then_some(args[2])
}

pub fn referenced_skill(args: &[i32]) -> Option<i32> {
    supports(args).then_some(args[3])
}

pub fn grant_transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    super::changed_features(managers, event, BuffActKind::ContractCastChannel)
        .into_iter()
        .filter_map(|(feature, _)| grant_rule_op(managers, &feature).map(|op| (feature, op)))
        .collect()
}

fn grant_rule_op(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<RuleOp> {
    let args = feature.values.get(1..)?;
    if !supports(args) {
        return None;
    }
    let [1, _, lock_buff_id, _] = args else {
        return None;
    };
    let bound_uid = managers.contract.bound_uid(feature.owner_uid)?;
    if managers.hp.current(bound_uid) <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
        BuffGrant {
            origin: super::feature_command_origin(feature)?,
            source_uid: feature.owner_uid,
            target_uid: bound_uid,
            buff_id: *lock_buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        },
    ))))
}

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::ContractCastChannel)
        || event.kind() != EventKind::RoundStart
        || !supports(&subscriber.args)
    {
        return None;
    }
    let [1, rate, lock_buff_id, owner_skill_id] = subscriber.args.as_slice() else {
        return None;
    };
    let Some(bound_uid) = managers.contract.bound_uid(subscriber.owner_uid) else {
        return Some(Vec::new());
    };
    if managers.hp.current(subscriber.owner_uid) <= 0 || managers.hp.current(bound_uid) <= 0 {
        return Some(Vec::new());
    }
    let Some(bound_skill_id) = managers
        .entity
        .snapshot(bound_uid)
        .and_then(|entity| entity.ex_skill)
        .filter(|skill_id| *skill_id > 0)
    else {
        return Some(Vec::new());
    };
    let Some(lock_buff_uid) = managers.buff.buff_id_uid(bound_uid, *lock_buff_id) else {
        return Some(Vec::new());
    };
    let origin = super::command_origin(subscriber)?;
    let current_hp = managers.hp.current(bound_uid).max(0);
    let loss =
        (i64::from(current_hp) * i64::from(*rate) / 1000).clamp(0, i64::from(i32::MAX)) as i32;
    let mut ops = Vec::with_capacity(5);
    if loss > 0 {
        ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            HpLoss {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: bound_uid,
                amount: loss,
                config_effect: 0,
                hurt: Some(HurtInfoData {
                    from_uid: subscriber.owner_uid,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 0,
                    damage_from: HurtDamageFromType::Buff,
                    buff_act_id: subscriber.key.definition.opcode,
                    buff_uid: subscriber.buff_uid,
                    hurt_effect_type: 0,
                    display_amount: None,
                }),
            },
        ))));
    }
    ops.push(RuleOp::Skill(invocation(
        bound_uid,
        bound_skill_id,
        catalog,
    )));
    ops.push(RuleOp::Skill(invocation(
        subscriber.owner_uid,
        *owner_skill_id,
        catalog,
    )));
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
        BuffRemove {
            origin,
            target_uid: bound_uid,
            selector: BuffRemoveSelector::Uid(lock_buff_uid),
        },
    ))));
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
        BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        },
    ))));
    Some(ops)
}

pub(crate) fn carrier_removal_rule_ops(
    managers: &BattleManagers,
    owner_uid: i64,
    origin: crate::engine::skill::rule::CommandOrigin,
) -> Vec<RuleOp> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(feature, BuffActKind::ContractCastChannel)
        })
        .map(|feature| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                origin,
                target_uid: owner_uid,
                selector: BuffRemoveSelector::Uid(feature.buff_uid),
            })))
        })
        .collect()
}

fn invocation(owner_uid: i64, skill_id: i32, catalog: &SkillEffectCatalog) -> SkillInvocation {
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: owner_uid,
        skill_id,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.extra_skill_kind = skill_kind_from_is_extra(catalog.extra_kind(skill_id));
    invocation
}

#[cfg(test)]
mod tests;
