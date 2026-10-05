use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffRemove, BuffRemoveSelector},
        card::{CardCommand, CardConsumeForEffect},
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
                ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
                    BuffRemove {
                        origin: super::command_origin(subscriber)?,
                        target_uid: subscriber.owner_uid,
                        selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
                    },
                ))));
            }
            Some(ops)
        }
        _ => Some(Vec::new()),
    }
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
mod tests {
    use super::*;
    use crate::engine::{event::subscription::SubscriptionKey, skill::rule::DefinitionKey};

    #[test]
    fn round_end_casts_the_configured_continuation_skill() {
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                DefinitionKey::new(862, "PaperCircleContinueChannel"),
            ),
            act_type: "PaperCircleContinueChannel".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![31050152, 3, 210, 2, 3, 4, 31050181, 31050182, 31050183],
            raw: "862#31050152#3#210#2,3,4#31050181,31050182,31050183".to_owned(),
        };

        assert!(matches!(
            rule_ops(
                &crate::engine::manager::BattleManagers::default(),
                &subscriber,
                &BattleEvent::Kind(EventKind::RoundEnd)
            )
            .as_deref(),
            Some([RuleOp::Skill(SkillInvocation {
                plan: SkillRequest {
                    source_uid: 10,
                    skill_id: 31050152,
                },
                ..
            })])
        ));
    }

    fn channel_fight(channel_duration: i32) -> sonettobuf::Fight {
        sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3105),
                    current_hp: Some(100),
                    buffs: vec![
                        sonettobuf::BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31050131),
                            from_uid: Some(10),
                            duration: Some(channel_duration),
                            ..Default::default()
                        },
                        sonettobuf::BuffInfo {
                            uid: Some(21),
                            buff_id: Some(31050143),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn channel_subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31050131,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                DefinitionKey::new(862, "PaperCircleContinueChannel"),
            ),
            act_type: "PaperCircleContinueChannel".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![31050151, 3, 210, 1, 2, 3, 31050141, 31050142, 31050143],
            raw: "862#31050151#3#210#1,2,3#31050141,31050142,31050143".to_owned(),
        }
    }

    #[test]
    fn channel_end_grants_moxie_for_the_current_force_field_and_ends_the_channel() {
        crate::test_support::init_config();
        let event = BattleEvent::Kind(EventKind::RoundEnd);
        let ending = BattleManagers::seeded(&channel_fight(1));
        let continuing = BattleManagers::seeded(&channel_fight(2));

        assert!(matches!(
            rule_ops(&ending, &channel_subscriber(), &event).as_deref(),
            Some([
                RuleOp::Skill(_),
                RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                    ExPointChange {
                        target_uid: 10,
                        delta: 3,
                        ..
                    }
                ))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    selector: BuffRemoveSelector::Uid(20),
                    ..
                })))
            ])
        ));
        assert!(matches!(
            rule_ops(&continuing, &channel_subscriber(), &event).as_deref(),
            Some([RuleOp::Skill(_)])
        ));
    }
}
