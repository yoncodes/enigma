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
            RuleOp::Command(BattleCommand::Buff(BuffCommand::EndChannel(BuffRemove {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                    key: crate::engine::skill::rule::DefinitionKey { opcode: 0, .. },
                },
                selector: BuffRemoveSelector::Uid(20),
                ..
            })))
        ])
    ));
    let scoped = scoped_rule_ops(&ending, &channel_subscriber(), &event).unwrap();
    assert!(scoped[..2].iter().all(|op| {
        op.scope == super::super::BuffActFrameScope::SubscriberFrame && op.group_with_siblings
    }));
    assert_eq!(
        scoped[2].scope,
        super::super::BuffActFrameScope::SubscriberFrame
    );
    assert!(!scoped[2].group_with_siblings);
    assert!(matches!(
        rule_ops(&continuing, &channel_subscriber(), &event).as_deref(),
        Some([RuleOp::Skill(_)])
    ));
}
