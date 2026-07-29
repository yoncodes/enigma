use std::collections::BTreeMap;

use crate::engine::{
    event::payload::BattleEvent,
    manager::buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    skill::{
        buff_act::registry::{self, BuffActKind},
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
    pool: &crate::engine::skill::target::TargetPool,
) -> Option<Vec<RuleOp>> {
    let (target_uid, target_uids, is_attack) = match event {
        BattleEvent::SkillAction(action) => (
            action.target_uid,
            action.target_uids.as_slice(),
            action.is_attack,
        ),
        BattleEvent::AllyAction(action) => (
            action.target_uid,
            action.target_uids.as_slice(),
            catalog.is_attack(action.skill_id),
        ),
        BattleEvent::Hit(hit) => (hit.target_uid, &[][..], true),
        _ => return None,
    };
    if (target_uid == 0 && target_uids.is_empty()) || !supports_skill(subscriber, is_attack) {
        return Some(Vec::new());
    }
    let (_, buff_ids) = subscriber.args.split_first()?;
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
            ops.push(RuleOp::Command(BattleCommand::Buff(command)));
        }
    }
    if ops.is_empty() {
        return Some(Vec::new());
    }
    if consumes_effect_count(subscriber) {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid),
                amount: 1,
                depleted: DepletedBuff::Remove,
            },
        ))));
    }
    Some(ops)
}

pub fn scoped_rule_ops(
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
    pool: &crate::engine::skill::target::TargetPool,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(subscriber, event, catalog, pool).map(|ops| {
        ops.into_iter()
            .map(super::BuffActRuleOp::subscriber_from_owner)
            .collect()
    })
}

fn matches(subscriber: &BuffActSubscriber) -> bool {
    matches!(
        registry::kind(subscriber.key.definition.opcode, &subscriber.act_type),
        Some(BuffActKind::AddToTarget | BuffActKind::AddToAttackTargets)
    )
}

fn consumes_effect_count(subscriber: &BuffActSubscriber) -> bool {
    registry::kind(subscriber.key.definition.opcode, &subscriber.act_type)
        == Some(BuffActKind::AddToAttackTargets)
        && config::try_get()
            .and_then(|db| db.skill_buff.get(subscriber.buff_id))
            .is_some_and(|buff| buff.effect_count > 0)
}

fn grant_command(
    subscriber: &BuffActSubscriber,
    target_uid: i64,
    buff_id: i32,
    layer: i32,
) -> Option<BuffCommand> {
    let source_uid = match registry::kind(subscriber.key.definition.opcode, &subscriber.act_type) {
        Some(BuffActKind::AddToAttackTargets) => subscriber.owner_uid,
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
mod tests {

    use super::*;
    use crate::engine::{
        event::{kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
        manager::buff::CommandOrigin,
        skill::{
            action::{SkillActionEvent, SkillExecutionMode, SkillPhase},
            effect::SkillEffectCatalog,
            rule::RuleDomain,
            target::TargetPool,
        },
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    #[test]
    fn repeated_target_buff_ids_become_layers() {
        let subscriber = BuffActSubscriber {
            owner_uid: 1,
            source_uid: 1,
            buff_uid: 2,
            buff_id: 31020111,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillCast,
                crate::engine::skill::rule::DefinitionKey::new(503, "AddToTarget"),
            ),
            act_type: "AddToTarget".to_owned(),
            effect_time: 208,
            effect_condition: 0,
            args: vec![0, 4150001, 4150001, 4150001],
            raw: "503#0#4150001#4150001#4150001".to_owned(),
        };

        let mut counts = BTreeMap::new();
        for &id in subscriber.args.iter().skip(1) {
            *counts.entry(id).or_insert(0) += 1;
        }
        assert_eq!(counts.get(&4150001), Some(&3));
    }

    #[test]
    fn exact_buff_act_produces_a_manager_owned_grant() {
        let subscriber = BuffActSubscriber {
            owner_uid: 1,
            source_uid: 10,
            buff_uid: 2,
            buff_id: 31020111,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillCast,
                crate::engine::skill::rule::DefinitionKey::new(503, "AddToTarget"),
            ),
            act_type: "AddToTarget".to_owned(),
            effect_time: 208,
            effect_condition: 0,
            args: vec![0, 4150001],
            raw: "503#0#4150001".to_owned(),
        };

        assert!(matches!(
            grant_command(&subscriber, -1, 4150001, 3),
            Some(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key,
                },
                source_uid: 10,
                target_uid: -1,
                buff_id: 4150001,
                amount: Some(3),
                occurrences: 1,
                child_uid_reservations: 0,
            })) if key.matches(503, "AddToTarget")
        ));
    }

    #[test]
    fn add_to_target_frames_are_owned_by_the_carrier() {
        crate::test_support::init_config();
        let subscriber = BuffActSubscriber {
            owner_uid: 1,
            source_uid: 1,
            buff_uid: 2,
            buff_id: 530000711,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(503, "AddToTarget"),
            ),
            act_type: "AddToTarget".to_owned(),
            effect_time: 208,
            effect_condition: 0,
            args: vec![0, 4101],
            raw: "503#0#4101".to_owned(),
        };
        let pool = TargetPool::from_fight(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: crate::engine::skill::rule::DefinitionKey::new(1, "TestSkill"),
            },
            source_uid: 1,
            target_uid: -1,
            skill_id: 1,
            amount: 1,
            shield_absorbed: 0,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            assassinate: false,
        });

        let ops =
            scoped_rule_ops(&subscriber, &event, &SkillEffectCatalog::default(), &pool).unwrap();

        assert!(!ops.is_empty());
        assert!(
            ops.iter()
                .all(|op| op.source == super::super::BuffActFrameSource::Owner)
        );
    }

    #[test]
    fn limited_attack_target_grant_consumes_one_configured_use() {
        crate::test_support::init_config();
        let subscriber = BuffActSubscriber {
            owner_uid: 99_998,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31_130_123,
            team_type: 1,
            owner_alive: true,
            amount: 3,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(928, "AddToTarget"),
            ),
            act_type: "AddToTarget".to_owned(),
            effect_time: 908,
            effect_condition: 0,
            args: vec![0, 31_130_122, 31_130_124],
            raw: "928#0#31130122#31130124".to_owned(),
        };
        let pool = TargetPool::from_fight(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(99_998),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let event = BattleEvent::SkillAction(SkillActionEvent {
            source_uid: 99_998,
            skill_id: 2_240_001,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: SkillPhase::AfterDamage,
            skill_slot: 0,
            is_attack: true,
            rank: 0,
            skill_type: 0,
            effect_tag: 2,
            assassinate: false,
            damage_amount: 100,
            kill_count: 0,
            crit_count: 1,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: SkillExecutionMode::Nested,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        });

        let ops = rule_ops(&subscriber, &event, &SkillEffectCatalog::default(), &pool).unwrap();

        assert!(ops.iter().any(|op| matches!(
            op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                source_uid: 99_998,
                target_uid: -1,
                ..
            })))
        )));

        assert!(matches!(
            ops.last(),
            Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                BuffConsume {
                    target_uid: 99_998,
                    selector: BuffSelector::Uid(20),
                    amount: 1,
                    depleted: DepletedBuff::Remove,
                    ..
                }
            ))))
        ));
    }
}
