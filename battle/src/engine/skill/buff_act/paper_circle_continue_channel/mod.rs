use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffRemove, BuffRemoveSelector},
        card::{CardCommand, CardConsumeForEffect, CardConsumptionKind},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::PaperCircleContinueChannel) {
        return None;
    }
    match event {
        BattleEvent::Kind(EventKind::RoundEnd) => {
            let skill_id = referenced_skill(&subscriber.raw)?;
            let mut invocation = SkillInvocation::from(SkillRequest {
                source_uid: subscriber.owner_uid,
                skill_id,
            });
            invocation.target_observed_extra_action = Some(false);
            let mut ops = vec![RuleOp::Skill(invocation)];
            if channel_ending(managers, subscriber.owner_uid, subscriber.buff_uid) {
                ops.extend(channel_end_moxie(managers, subscriber));
                ops.push(RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::EndChannel(BuffRemove {
                        origin: crate::engine::skill::rule::CommandOrigin {
                            domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                            key: crate::engine::skill::rule::DefinitionKey::new(
                                0,
                                "PaperCircleChannelEnd",
                            ),
                        },
                        target_uid: subscriber.owner_uid,
                        selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
                    }),
                )));
            }
            Some(ops)
        }
        _ => Some(Vec::new()),
    }
}

pub fn scoped_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    let ops = rule_ops(managers, subscriber, event)?;
    let last = ops.len().saturating_sub(1);
    Some(
        ops.into_iter()
            .enumerate()
            .map(|(index, op)| {
                if index == last && matches!(op, RuleOp::Command(BattleCommand::Buff(_))) {
                    super::BuffActRuleOp::separate_subscriber_from_owner(op)
                } else {
                    super::BuffActRuleOp::subscriber_from_owner(op)
                }
            })
            .collect(),
    )
}

// "At the start of the round, recovers all Eureka, converts all own incantations into the
// configured skill, and casts them all": one cast per converted incantation.
pub fn setup_rule_ops(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
) -> Option<Vec<RuleOp>> {
    if !super::is_kind(feature, BuffActKind::PaperCircleContinueChannel) {
        return None;
    }
    let (skill_id, target_rule) = conversion(&feature.raw)?;
    let origin = super::feature_command_origin(feature)?;
    let owner_uid = feature.owner_uid;
    let eureka = managers.eureka.get(owner_uid, EUREKA_RESOURCE_ID);
    let cards = managers.card.plan_effect_consumption(owner_uid);
    let mut ops = Vec::with_capacity(cards.len() + 2);
    if eureka.max != eureka.current {
        ops.push(RuleOp::Command(BattleCommand::Eureka(
            EurekaCommand::Change(EurekaChange {
                origin,
                source_uid: owner_uid,
                target_uid: owner_uid,
                power_id: EUREKA_RESOURCE_ID,
                delta: eureka.max - eureka.current,
                effect_type: EffectType::Powerchange as i32,
            }),
        )));
    }
    if !cards.is_empty() {
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::ConsumeForEffect(CardConsumeForEffect {
                origin,
                owner_uid,
                indices: cards.iter().map(|(index, _)| *index).collect(),
                kind: CardConsumptionKind::PaperCircle,
            }),
        )));
    }
    ops.extend(cards.into_iter().map(|_| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: owner_uid,
            skill_id,
        }
        .into();
        invocation.target = SkillTarget::LogicRule(target_rule);
        invocation.mode = SkillExecutionMode::Active;
        invocation.target_observed_extra_action = Some(false);
        RuleOp::Skill(invocation)
    }));
    Some(ops)
}

// "When the channel status ends, grants Moxie +1/2/3 to self based on the current Gust Force
// Field".
fn channel_end_moxie(managers: &BattleManagers, subscriber: &BuffActSubscriber) -> Option<RuleOp> {
    let owner_uid = subscriber.owner_uid;
    let mut fields = subscriber.raw.split('#').skip(4);
    let levels = fields.next()?.split(',');
    let field_ids = fields.next()?.split(',');
    let delta = levels
        .zip(field_ids)
        .find(|(_, field_id)| {
            field_id
                .trim()
                .parse::<i32>()
                .is_ok_and(|field_id| managers.buff.has_buff_id(owner_uid, field_id))
        })?
        .0
        .trim()
        .parse::<i32>()
        .ok()?;
    Some(RuleOp::Command(BattleCommand::ExPoint(
        ExPointCommand::Change(ExPointChange {
            origin: super::command_origin(subscriber)?,
            source_uid: owner_uid,
            target_uid: owner_uid,
            delta,
            config_effect: 0,
            effect_type: EffectType::Expointchange as i32,
        }),
    )))
}

// The channel's last round-end tick runs while one round of duration remains; the act ends the
// channel there.
fn channel_ending(managers: &BattleManagers, owner_uid: i64, buff_uid: i64) -> bool {
    managers
        .buff
        .snapshot(owner_uid, buff_uid)
        .is_some_and(|buff| buff.duration == Some(1))
}

// Fields: act # skill # value # conversion target rule # levels # force fields.
fn conversion(raw: &str) -> Option<(i32, i32)> {
    let skill_id = referenced_skill(raw)?;
    let target_rule = raw.split('#').nth(3)?.parse::<i32>().ok()?;
    crate::engine::skill::target::is_mapped_target_code(target_rule)
        .then_some((skill_id, target_rule))
}

pub fn referenced_skill(raw: &str) -> Option<i32> {
    let fields = raw.split('#').collect::<Vec<_>>();
    let [act, skill, value0, value1, levels, field_ids] = fields.as_slice() else {
        return None;
    };
    if act.parse::<i32>().ok()? != 862
        || value0.parse::<i32>().is_err()
        || value1.parse::<i32>().is_err()
    {
        return None;
    }
    let parse_group = |raw: &str| {
        raw.split(',')
            .map(str::trim)
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()
    };
    let levels = parse_group(levels)?;
    let field_ids = parse_group(field_ids)?;
    let skill_id = skill.parse::<i32>().ok()?;
    (skill_id > 0 && !levels.is_empty() && levels.len() == field_ids.len()).then_some(skill_id)
}

#[cfg(test)]
mod tests;
