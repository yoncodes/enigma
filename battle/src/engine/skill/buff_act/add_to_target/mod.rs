use std::collections::BTreeMap;

use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{
        BuffCommand, BuffConsume, BuffGrant, BuffRemove, BuffRemoveSelector, BuffSelector,
        DepletedBuff,
    },
    skill::{
        buff_act::registry::{self, BuffActKind},
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &crate::engine::manager::BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
    pool: &crate::engine::skill::target::TargetPool,
) -> Option<Vec<RuleOp>> {
    let kind = registry::kind(subscriber.key.definition.opcode, &subscriber.act_type)?;
    let (target_uid, target_uids, is_attack) = match (kind, event) {
        (BuffActKind::AddToAttacker, BattleEvent::Hit(hit)) => {
            if hit.target_uid != subscriber.owner_uid {
                return Some(Vec::new());
            }
            (hit.source_uid, &[][..], true)
        }
        (_, BattleEvent::SkillAction(action)) => (
            action.target_uid,
            action.target_uids.as_slice(),
            action.is_attack,
        ),
        (_, BattleEvent::AllyAction(action)) => (
            action.target_uid,
            action.target_uids.as_slice(),
            catalog.is_attack(action.skill_id),
        ),
        (_, BattleEvent::Hit(hit)) => (hit.target_uid, &[][..], true),
        _ => return None,
    };
    if (target_uid == 0 && target_uids.is_empty()) || !supports_skill(subscriber, is_attack) {
        return Some(Vec::new());
    }
    let buff_ids = match kind {
        BuffActKind::AddToAttacker => subscriber.args.as_slice(),
        _ => subscriber.args.get(1..)?,
    };
    let mut counts = BTreeMap::new();
    for &buff_id in buff_ids.iter().filter(|buff_id| **buff_id > 0) {
        *counts.entry(buff_id).or_insert(0) += 1;
    }
    let targets = if target_uids.is_empty() {
        vec![target_uid]
    } else {
        target_uids.to_vec()
    }
    .into_iter()
    .filter(|target_uid| {
        !is_attack
            || pool.source_is_attacker(*target_uid) != pool.source_is_attacker(subscriber.owner_uid)
    })
    .collect::<Vec<_>>();
    let marker_effect_type =
        super::wire::find(subscriber.key.definition.opcode, &subscriber.act_type)
            .and_then(|wire| wire.markers(super::wire::WirePhase::Add).first())
            .copied();
    let mut ops = Vec::new();
    for target_uid in targets {
        for (&buff_id, &layer) in &counts {
            let Some(command) = grant_command(subscriber, target_uid, buff_id, layer) else {
                continue;
            };
            if let Some(effect_type) = marker_effect_type {
                ops.push(RuleOp::BuffFeatureMarker {
                    target_uid: subscriber.owner_uid,
                    effect_type,
                    effect_num: subscriber.buff_id,
                    buff_act_id: subscriber.key.definition.opcode,
                });
            }
            if kind == BuffActKind::AddToAttacker {
                ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
                    BuffRemove {
                        origin: super::command_origin(subscriber)?,
                        target_uid,
                        selector: BuffRemoveSelector::ExactId(buff_id),
                    },
                ))));
            }
            ops.push(RuleOp::Command(BattleCommand::Buff(command)));
        }
    }
    if ops.is_empty() {
        return Some(Vec::new());
    }
    if consumes_effect_count(managers, subscriber) {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::ConsumeAttackCharge(BuffConsume {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid),
                amount: 1,
                depleted: DepletedBuff::Remove,
            }),
        )));
    }
    Some(ops)
}

pub fn scoped_rule_ops(
    managers: &crate::engine::manager::BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
    pool: &crate::engine::skill::target::TargetPool,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(managers, subscriber, event, catalog, pool).map(|ops| {
        ops.into_iter()
            .map(super::BuffActRuleOp::subscriber_from_owner)
            .collect()
    })
}

fn matches(subscriber: &BuffActSubscriber) -> bool {
    matches!(
        registry::kind(subscriber.key.definition.opcode, &subscriber.act_type),
        Some(
            BuffActKind::AddToAttacker | BuffActKind::AddToTarget | BuffActKind::AddToAttackTargets
        )
    )
}

fn consumes_effect_count(
    managers: &crate::engine::manager::BattleManagers,
    subscriber: &BuffActSubscriber,
) -> bool {
    registry::kind(subscriber.key.definition.opcode, &subscriber.act_type)
        == Some(BuffActKind::AddToAttackTargets)
        && managers.catalog().buff_has_effect_count(subscriber.buff_id)
}

fn grant_command(
    subscriber: &BuffActSubscriber,
    target_uid: i64,
    buff_id: i32,
    layer: i32,
) -> Option<BuffCommand> {
    let source_uid = match registry::kind(subscriber.key.definition.opcode, &subscriber.act_type) {
        Some(BuffActKind::AddToAttacker | BuffActKind::AddToAttackTargets) => subscriber.owner_uid,
        _ => subscriber.source_uid,
    };
    Some(BuffCommand::Grant(BuffGrant {
        origin: super::command_origin(subscriber)?,
        source_uid,
        target_uid,
        buff_id,
        amount: (layer > 1).then_some(layer),
        occurrences: 1,
        child_uid_reservations: 0,
    }))
}

pub fn supports_skill(subscriber: &BuffActSubscriber, is_attack: bool) -> bool {
    !matches(subscriber) || is_attack
}

#[cfg(test)]
mod tests;
