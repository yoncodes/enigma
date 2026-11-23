use super::*;
use crate::engine::{
    event::{kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
    manager::hp::HurtDamageFromType,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

fn subscriber(buff_id: i32) -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::TargetAttacked,
            DefinitionKey::new(510, "DamageNotMoreThan"),
        ),
        act_type: "DamageNotMoreThan".to_owned(),
        effect_time: 207,
        effect_condition: 0,
        args: vec![100],
        raw: "510#100".to_owned(),
    }
}

fn hit(target_uid: i64) -> BattleEvent {
    BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "Damage"),
        },
        source_uid: -1,
        target_uid,
        skill_id: 1,
        amount: 100,
        resolved_damage: 100,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

#[test]
fn stacked_cap_consumes_one_layer_after_its_owner_is_hit() {
    crate::test_support::init_config();

    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(20),
                    buff_id: Some(6240530),
                    from_uid: Some(10),
                    count: Some(1),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);

    assert!(
        consume_after_hit(&managers, &subscriber(6240530), &hit(11))
            .unwrap()
            .is_empty()
    );
    assert!(
        consume_after_hit(&managers, &subscriber(610091), &hit(10))
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        consume_after_hit(&managers, &subscriber(6240530), &hit(10))
            .unwrap()
            .as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                target_uid: 10,
                selector: BuffSelector::Uid(20),
                amount: 1,
                depleted: DepletedBuff::Remove,
                ..
            }
        )))]
    ));

    let RuleOp::Command(BattleCommand::Buff(command)) =
        consume_after_hit(&managers, &subscriber(6240530), &hit(10))
            .unwrap()
            .pop()
            .unwrap()
    else {
        panic!("expected buff consumption");
    };
    let changes = managers.buff.execute(&managers.hp, command).unwrap();

    assert_eq!(changes.change.refreshed[0].before.layer, Some(3));
    assert_eq!(changes.change.refreshed[0].after.layer, Some(2));
    assert_eq!(changes.change.refreshed[0].after.count, Some(1));
}
