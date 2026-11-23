use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::BuffAct,
    key: DefinitionKey::new(953, "BloodPoolTag"),
};

#[test]
fn hp_loss_uses_the_current_bloodtithe_limit_for_conversion() {
    let mut managers = BattleManagers::default();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            key(1),
            GaugeOperation::Enable { max: Some(56) },
        ))
        .unwrap();
    managers
        .gauge
        .execute_command(GaugeCommand::new(
            ORIGIN,
            key(1),
            GaugeOperation::ChangeMax { delta: 28 },
        ))
        .unwrap();

    assert!(matches!(
        hp_loss_event_rule_op(&managers, ORIGIN, 1, 10, 2_940, 20).as_slice(),
        [RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
            operation: GaugeOperation::AccumulateValue {
                amount: 2_940,
                threshold: 2_940,
            },
            ..
        }))]
    ));
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
