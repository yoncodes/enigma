use super::*;
use crate::{
    engine::{event::subscription::SubscriptionKey, skill::rule::DefinitionKey},
    test_support::init_config,
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn exact_feature_emits_a_configured_temporary_card_command() {
    let feature = ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "AddSpTempCard".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "815#999".to_owned(),
        values: vec![815, 999],
    };

    assert!(matches!(
        rule_op(&feature, 10, 40),
        Some(RuleOp::Command(BattleCommand::Card(
            CardCommand::AddTemporary(CardAddTemporary {
                target_uid: 10,
                skill_id: 999,
                reserve_id: 40,
                team_type: 1,
                ..
            })
        )))
    ));
}

#[test]
fn round_start_card_adds_one_card_per_layer_before_consuming_the_buff() {
    init_config();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        team_type: 1,
        owner_alive: true,
        amount: 2,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(815, "AddSpTempCard"),
        ),
        act_type: "AddSpTempCard".to_owned(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![999],
        raw: "815#999".to_owned(),
    };

    let ops = subscriber_rule_ops(
        &BattleManagers::seeded(&Fight::default()),
        &subscriber,
        &BattleEvent::Kind(EventKind::RoundStartCard),
        3114,
    )
    .unwrap();

    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))
            if add.skill_id == 999 && add.target_uid == 10 && add.reserve_id == 3114
    ));
    assert!(matches!(
        &ops[1].op,
        RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))
            if add.skill_id == 999 && add.target_uid == 10 && add.reserve_id == 3114
    ));
    assert!(matches!(
        &ops[2].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::RemoveAfterTrigger(remove)))
            if remove.target_uid == 10
                && remove.selector == BuffRemoveSelector::Uid(20)
    ));
    assert_eq!(
        ops[0].scope,
        super::super::BuffActFrameScope::SubscriberFrame
    );
    assert_eq!(ops[0].source, super::super::BuffActFrameSource::Owner);
    assert_eq!(ops[1].source, super::super::BuffActFrameSource::Owner);
    assert!(!ops[2].group_with_siblings);
    assert_eq!(ops[2].frame_owner, super::super::BuffActFrameOwner::Command);
}

#[test]
fn hero_card_group_and_rank_select_the_captured_precast() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3070),
                skill_group1: vec![307001172, 307001182, 307001192],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 307002612,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(739, "CreateHeroTempCards"),
        ),
        act_type: "CreateHeroTempCards".to_owned(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![1, 2, 1],
        raw: "739#1#2#1".to_owned(),
    };

    let ops = hero_skill_subscriber_rule_ops(
        &pool,
        &subscriber,
        &BattleEvent::Kind(EventKind::RoundStartCard),
    )
    .unwrap();

    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))
            if add.skill_id == 307001182
                && add.target_uid == 10
                && add.reserve_id == 3070
    ));
    assert!(!supports_hero_skill(&[0, 2, 1]));
    assert!(!supports_hero_skill(&[1, 2, 2]));
}

#[test]
fn configured_skill3_card_uses_the_hero_temporary_lane_and_consumes_its_carrier() {
    let subscriber = BuffActSubscriber {
        owner_uid: 237352626,
        source_uid: 237352626,
        buff_uid: 1622,
        buff_id: 312451467,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(10015, "CreateTempSkill3Card"),
        ),
        act_type: "CreateTempSkill3Card".to_owned(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![312451036, 1],
        raw: "10015#312451036#1".to_owned(),
    };

    let ops = configured_skill3_subscriber_rule_ops(
        &subscriber,
        &BattleEvent::Kind(EventKind::RoundStartCard),
        3124,
    )
    .unwrap();

    assert!(matches!(
        &ops[0].op,
        RuleOp::Command(BattleCommand::Card(CardCommand::AddTemporary(add)))
            if add.target_uid == 237352626
                && add.skill_id == 312451036
                && add.hero_id == Some(3124)
                && add.reserve_id == 0
                && add.team_type == 1
                && add.kind == TemporaryCardKind::ConfiguredSkill3
    ));
    assert!(matches!(
        &ops[1].op,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::RemoveAfterTrigger(remove)))
            if remove.target_uid == 237352626
                && remove.selector == BuffRemoveSelector::Uid(1622)
    ));
    assert!(!supports_configured_skill3(&[312451036, 2]));
    assert_eq!(ops[0].source, super::super::BuffActFrameSource::Owner);
    assert!(!ops[1].group_with_siblings);
}
