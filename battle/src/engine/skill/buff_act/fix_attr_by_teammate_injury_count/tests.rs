use super::*;
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn tracked_injuries_drive_the_dynamic_attribute_count() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let tracker = managers.buff.add(&managers.hp, 10, 10, 433011, 0);
    let tracker_uid = tracker.unwrap().buff.uid.unwrap();
    managers
        .buff
        .execute(
            &managers.hp,
            BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                origin: crate::engine::skill::rule::CommandOrigin {
                    domain: crate::engine::skill::rule::RuleDomain::BuffAct,
                    key: crate::engine::skill::rule::DefinitionKey::new(800, "TeammateInjuryCount"),
                },
                target_uid: 10,
                buff_uid: tracker_uid,
                act_id: 800,
                delta: 2,
            }),
        )
        .unwrap();
    let feature = ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 1,
        buff_id: 433031,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "FixAttrByTeammateInjuryCountNotReset".to_owned(),
        effect_time: 210,
        effect_condition: 0,
        raw: "801#214#9#90".to_owned(),
        values: vec![801, 214, 9, 90],
    };

    assert_eq!(
        attribute_delta(&feature, AttrId::IncantationMight, &managers.buff, true,),
        18
    );
    assert_eq!(
        attribute_delta(&feature, AttrId::IncantationMight, &managers.buff, false),
        9
    );
    assert_eq!(
        attribute_delta(&feature, AttrId::DmgBonus, &managers.buff, true),
        0
    );
}

#[test]
fn exact_registry_entries_describe_the_tracker_and_dynamic_attribute() {
    let tracker = super::super::registry::find(800, "TeammateInjuryCount").unwrap();
    assert_eq!(
        tracker.kind,
        super::super::registry::BuffActKind::TeammateInjuryCount
    );
    assert!(tracker.runtime.effect_time_subscription);
    assert!(tracker.runtime.handler.is_some());
    assert_eq!(
        tracker.runtime.actor_scope,
        super::super::registry::RuntimeActorScope::Team
    );
    assert!(super::super::registry::has_destination(
        800,
        "TeammateInjuryCount",
        &[]
    ));

    let attribute =
        super::super::registry::find(801, "FixAttrByTeammateInjuryCountNotReset").unwrap();
    assert_eq!(
        attribute.state.read_timing,
        super::super::registry::StatReadTiming::OnTrigger
    );
    assert!(super::super::registry::has_destination(
        801,
        "FixAttrByTeammateInjuryCountNotReset",
        &[214, 9, 90]
    ));
}
