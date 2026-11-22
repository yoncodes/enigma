use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    fight::versions::{RoundStartSetupLayout, round_start_setup_layout},
    manager::{
        BattleManagers,
        buff::{
            ActiveBuffFeature, BuffCommand, BuffDurationAdvance, BuffRemove, BuffRemoveSelector,
        },
        card::{CardAddTemporary, CardCommand, TemporaryCardKind},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

use super::{effect_time::ROUND_START_CARD_SETTLED_DURATION, is_kind, registry::BuffActKind};

pub(crate) fn opening_duration(
    managers: &BattleManagers,
    owner_uids: &[i64],
) -> Option<BuffDurationAdvance> {
    if owner_uids.is_empty() {
        return None;
    }
    let stage = ROUND_START_CARD_SETTLED_DURATION;
    let buff_uids = managers
        .buff
        .duration_targets(stage, owner_uids)
        .into_iter()
        .filter(|timer| {
            managers
                .buff
                .buff_has_act(timer.owner_uid, timer.buff_uid, BuffActKind::AddSpTempCard)
        })
        .map(|timer| timer.buff_uid)
        .collect::<Vec<_>>();
    if buff_uids.is_empty() {
        return None;
    }
    BuffDurationAdvance::new(stage, owner_uids.to_vec(), Some(buff_uids))
}

pub fn skill_id(feature: &ActiveBuffFeature) -> Option<i32> {
    if !is_kind(feature, BuffActKind::AddSpTempCard) {
        return None;
    }
    let [_, skill_id, ..] = feature.values.as_slice() else {
        return None;
    };
    (*skill_id > 0).then_some(*skill_id)
}

pub fn rule_op(feature: &ActiveBuffFeature, target_uid: i64, reserve_id: i64) -> Option<RuleOp> {
    Some(RuleOp::Command(BattleCommand::Card(
        CardCommand::AddTemporary(CardAddTemporary {
            origin: super::feature_command_origin(feature)?,
            target_uid,
            skill_id: skill_id(feature)?,
            hero_id: None,
            reserve_id,
            team_type: feature.team_type,
            kind: TemporaryCardKind::ConfiguredSkill,
        }),
    )))
}

pub fn subscriber_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    reserve_id: i64,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AddSpTempCard)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
    {
        return None;
    }
    let [skill_id, ..] = subscriber.args.as_slice() else {
        return None;
    };
    if *skill_id <= 0 || reserve_id <= 0 {
        return None;
    }
    let mut ops = temporary_cards(
        subscriber,
        *skill_id,
        None,
        reserve_id,
        TemporaryCardKind::ConfiguredSkill,
        subscriber.amount,
    )?;
    if round_start_setup_layout(managers.fight_version()) != Some(RoundStartSetupLayout::Version7)
        || managers
            .buff
            .duration_stage(subscriber.owner_uid, subscriber.buff_uid)
            != Some(ROUND_START_CARD_SETTLED_DURATION)
    {
        ops.push(remove_after_trigger(subscriber)?);
    }
    Some(ops)
}

pub fn supports_hero_skill(args: &[i32]) -> bool {
    matches!(args, [group, rank, 1] if matches!(group, 1 | 2) && (1..=3).contains(rank))
}

pub fn supports_configured_skill3(args: &[i32]) -> bool {
    matches!(args, [skill_id, 1] if *skill_id > 0)
}

pub fn configured_skill3_subscriber_rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    hero_id: i32,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::CreateTempSkill3Card)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
        || !supports_configured_skill3(&subscriber.args)
    {
        return None;
    }
    temporary_card_rule_ops(
        subscriber,
        subscriber.args[0],
        Some(hero_id),
        0,
        TemporaryCardKind::ConfiguredSkill3,
        1,
    )
}

pub fn hero_skill_subscriber_rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::CreateHeroTempCards)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
        || !supports_hero_skill(&subscriber.args)
    {
        return None;
    }
    let [group, rank, 1] = subscriber.args.as_slice() else {
        return None;
    };
    let owner = pool.entity(subscriber.owner_uid)?;
    let skills = if *group == 1 {
        &owner.skill_group1
    } else {
        &owner.skill_group2
    };
    let skill_id = skills
        .iter()
        .copied()
        .find(|skill_id| pool.catalog().skill_rank(*skill_id) == *rank)?;
    temporary_card_rule_ops(
        subscriber,
        skill_id,
        None,
        i64::from(owner.model_id),
        TemporaryCardKind::HeroSkill,
        1,
    )
}

fn temporary_card_rule_ops(
    subscriber: &BuffActSubscriber,
    skill_id: i32,
    hero_id: Option<i32>,
    reserve_id: i64,
    kind: TemporaryCardKind,
    count: i32,
) -> Option<Vec<super::BuffActRuleOp>> {
    let mut ops = temporary_cards(subscriber, skill_id, hero_id, reserve_id, kind, count)?;
    ops.push(remove_after_trigger(subscriber)?);
    Some(ops)
}

fn temporary_cards(
    subscriber: &BuffActSubscriber,
    skill_id: i32,
    hero_id: Option<i32>,
    reserve_id: i64,
    kind: TemporaryCardKind,
    count: i32,
) -> Option<Vec<super::BuffActRuleOp>> {
    let origin = super::command_origin(subscriber)?;
    let count = usize::try_from(count).ok().filter(|count| *count > 0)?;
    let mut ops = Vec::with_capacity(count + 1);
    ops.extend((0..count).map(|_| {
        super::BuffActRuleOp::subscriber_from_owner(RuleOp::Command(BattleCommand::Card(
            CardCommand::AddTemporary(CardAddTemporary {
                origin,
                target_uid: subscriber.owner_uid,
                skill_id,
                hero_id,
                reserve_id,
                team_type: subscriber.team_type,
                kind,
            }),
        )))
    }));
    Some(ops)
}

fn remove_after_trigger(subscriber: &BuffActSubscriber) -> Option<super::BuffActRuleOp> {
    let origin = super::command_origin(subscriber)?;
    Some(super::BuffActRuleOp::separate_independent_command(
        RuleOp::Command(BattleCommand::Buff(BuffCommand::RemoveAfterTrigger(
            BuffRemove {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
            },
        ))),
    ))
}

#[cfg(test)]
mod tests;
