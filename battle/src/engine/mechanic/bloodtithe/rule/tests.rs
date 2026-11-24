use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::BuffAct,
    key: DefinitionKey::new(953, "BloodPoolTag"),
};

#[test]
fn hp_loss_carries_progress_at_the_configured_threshold_when_the_limit_grows() {
    let mut managers = BattleManagers::default();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            key(1),
            GaugeOperation::Enable { max: Some(58) },
        ))
        .unwrap();
    assert_eq!(
        apply_loss(&mut managers, 9_714, HurtDamageFromType::SkillEffect),
        3
    );
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            key(1),
            GaugeOperation::ChangeValue { delta: 55 },
        ))
        .unwrap();

    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            key(1),
            GaugeOperation::ChangeMax { delta: 16 },
        ))
        .unwrap();
    assert_eq!(
        apply_loss(&mut managers, 2_194, HurtDamageFromType::SkillEffect),
        0
    );
    assert_eq!(
        apply_loss(&mut managers, 1_233, HurtDamageFromType::SkillEffect),
        1
    );
    assert_eq!(
        apply_loss(&mut managers, 1_426, HurtDamageFromType::SkillEffect),
        0
    );
    assert_eq!(
        apply_loss(&mut managers, 1_668, HurtDamageFromType::SkillEffect),
        1
    );
    assert_eq!(
        apply_loss(&mut managers, 1_501, HurtDamageFromType::SkillEffect),
        0
    );
}

#[test]
fn direct_hits_contribute_the_attack_credit_and_carry_into_the_next_round() {
    use HurtDamageFromType::{Buff, Skill, SkillEffect};

    let mut managers = BattleManagers::default();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            key(1),
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();

    for (loss, damage_from, gain) in [
        (9_070, SkillEffect, 3),
        (2_194, SkillEffect, 0),
        (3_486, SkillEffect, 1),
        (4_503, SkillEffect, 2),
        (4_278, SkillEffect, 1),
        (5_005, SkillEffect, 2),
        (813, Buff, 0),
        (1_050, Buff, 1),
        (998, Buff, 0),
        (1_167, Buff, 0),
        (813, SkillEffect, 1),
        (1_703, Skill, 0),
        (1_275, Skill, 1),
        (968, Skill, 0),
        (2_273, Buff, 1),
        (1_650, Buff, 1),
        (1_795, Buff, 0),
        (1_817, Buff, 1),
        (1_575, Buff, 0),
    ] {
        assert_eq!(
            apply_loss(&mut managers, loss, damage_from),
            gain,
            "{damage_from:?} loss {loss}"
        );
    }

    managers.begin_round();
    assert_eq!(apply_loss(&mut managers, 4_293, SkillEffect), 2);
}

fn apply_loss(managers: &mut BattleManagers, loss: i32, damage_from: HurtDamageFromType) -> i32 {
    let ops = hp_loss_event_rule_op(managers, ORIGIN, 1, 10, loss, damage_from, 0);
    let [RuleOp::Command(BattleCommand::Gauge(command))] = ops.as_slice() else {
        panic!("expected a Bloodtithe accumulation command");
    };
    managers
        .gauge
        .execute_command(*command)
        .unwrap()
        .applied_delta
}

fn tag_feature(owner_uid: i64, buff_uid: i64, buff_id: i32) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid,
        source_uid: owner_uid,
        buff_uid,
        buff_id,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "BloodPoolTag".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: "953".to_owned(),
        values: vec![953],
    }
}

fn team(heroes: &[(i64, i32, i32)]) -> (BattleManagers, TargetPool) {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        version: Some(7),
        attacker: Some(sonettobuf::FightTeam {
            entitys: heroes
                .iter()
                .map(
                    |&(uid, model_id, destiny_stone)| sonettobuf::FightEntityInfo {
                        uid: Some(uid),
                        model_id: Some(model_id),
                        team_type: Some(1),
                        current_hp: Some(100),
                        destiny_stone: Some(destiny_stone),
                        destiny_rank: Some(i32::from(destiny_stone > 0)),
                        ..Default::default()
                    },
                )
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    (
        BattleManagers::seeded(&fight),
        TargetPool::from_fight(&fight),
    )
}

#[test]
fn bloodtithe_tagged_allies_enable_the_gauge_whatever_marker_buff_they_carry() {
    let (managers, pool) = team(&[(10, 3120, 0), (11, 3126, 0)]);

    for marker in [tag_feature(10, 20, 6270501), tag_feature(11, 21, 31260221)] {
        assert!(matches!(
            enable_rule_op(&managers, &pool, &marker),
            Some(RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                key: GaugeKey {
                    kind: GaugeKind::Bloodtithe,
                    owner: GaugeOwner::Team(1),
                },
                operation: GaugeOperation::Enable { .. },
                ..
            })))
        ));
    }
}

#[test]
fn marker_without_a_bloodtithe_tagged_ally_does_not_enable_the_gauge() {
    let marker = tag_feature(10, 20, 6270501);

    let (managers, pool) = team(&[(10, 3088, 0)]);
    assert!(enable_rule_op(&managers, &pool, &marker).is_none());

    let (managers, pool) = team(&[(10, 3088, 308801)]);
    assert!(enable_rule_op(&managers, &pool, &marker).is_some());
}

#[test]
fn enabled_lingering_glow_blocks_the_bloodtithe_gauge() {
    let (mut managers, pool) = team(&[(10, 3120, 0)]);
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            GaugeKey {
                kind: GaugeKind::LingeringGlow,
                owner: GaugeOwner::Team(1),
            },
            GaugeOperation::Enable { max: Some(1) },
        ))
        .unwrap();

    assert!(enable_rule_op(&managers, &pool, &tag_feature(10, 20, 6270501)).is_none());
}
