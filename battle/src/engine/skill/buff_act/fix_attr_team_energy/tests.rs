use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    manager::gauge::{GaugeCommand, GaugeOperation},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

#[test]
fn team_inspiration_attribute_uses_the_configured_base_and_rate() {
    let feature = ActiveBuffFeature {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 1,
        buff_id: 31080143,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "FixAttrTeamEnergy".into(),
        effect_time: 0,
        effect_condition: 0,
        raw: "882#205#50#25".into(),
        values: vec![882, 205, 50, 25],
    };

    assert_eq!(
        attribute_delta(
            &feature,
            AttrId::DmgBonus,
            6,
            &BuffManager::default(),
            &TargetPool::default(),
        ),
        200
    );
}

#[test]
fn holder_scaled_variant_reads_the_committed_attribute_snapshot_once() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![sonettobuf::BuffInfo {
                    buff_id: Some(31080131),
                    uid: Some(1),
                    from_uid: Some(10),
                    act_common_params: Some("883#340".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature_kind(feature) == Some(BuffActKind::FixAttrTeamEnergyAndBuff))
        .unwrap();

    assert_eq!(
        attribute_delta(&feature, AttrId::Attack, 999, &managers.buff, &pool,),
        340
    );
}

#[test]
fn round_start_rule_snapshots_the_owned_team_energy_gauge() {
    let mut managers = BattleManagers::default();
    let key = crate::engine::mechanic::impromptu::team_energy_key(1);
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "Test"),
    };
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    let emitter_energy =
        crate::engine::mechanic::impromptu::inspiration_key(crate::engine::manager::emitter::UID);
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            emitter_energy,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            emitter_energy,
            GaugeOperation::ChangeValue { delta: 99 },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::ChangeValue { delta: 7 },
        ))
        .unwrap();
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 1,
        buff_id: 31080131,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStart,
            DefinitionKey::new(883, "FixAttrTeamEnergyAndBuff"),
        ),
        act_type: "FixAttrTeamEnergyAndBuff".into(),
        effect_time: 104,
        effect_condition: 0,
        args: vec![102, 300, 5, 150, 31080131],
        raw: "883#102#300#5#150#31080131".into(),
    };

    let ops = rule_ops(
        &managers,
        &TargetPool::default(),
        &subscriber,
        &crate::engine::event::payload::BattleEvent::RoundStart,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
            BuffSetState { params: Some(params), .. }
        )))] if params == "883#335"
    ));
}

#[test]
fn each_team_energy_variant_snapshots_its_own_encoded_value_on_grant() {
    crate::test_support::init_config();
    let mut managers = BattleManagers::default();
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "Test"),
    };
    let key = crate::engine::mechanic::impromptu::team_energy_key(1);
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::ChangeValue { delta: 10 },
        ))
        .unwrap();

    assert_eq!(
        grant_params(&managers, &TargetPool::default(), 31080143, 10, 1).as_deref(),
        Some("882#10")
    );
    assert_eq!(
        grant_params(&managers, &TargetPool::default(), 31080131, 10, 1).as_deref(),
        Some("883#350")
    );
}
