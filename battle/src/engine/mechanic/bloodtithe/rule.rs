use crate::engine::{
    event::payload::GaugeChangeEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        ex_point::{ExPointChange, ExPointCommand},
        gauge::{GaugeChangeKind, GaugeCommand, GaugeKey, GaugeKind, GaugeOperation, GaugeOwner},
    },
    skill::{
        buff_act,
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
        target::TargetPool,
    },
};

// character_battle_tag 111 "Bloodtithe"
const BLOODTITHE_BATTLE_TAG: i32 = 111;

pub const fn key(team: i32) -> GaugeKey {
    GaugeKey {
        kind: GaugeKind::Bloodtithe,
        owner: GaugeOwner::Team(team),
    }
}

pub fn enable_rule_op(
    managers: &BattleManagers,
    pool: &TargetPool,
    enabler: &ActiveBuffFeature,
) -> Option<RuleOp> {
    let team = enabler.team_type;
    if managers
        .gauge
        .get(GaugeKey {
            kind: GaugeKind::LingeringGlow,
            owner: GaugeOwner::Team(team),
        })
        .is_some()
    {
        return None;
    }
    if !managers
        .buff
        .alive_team_uids(team, &managers.hp)
        .into_iter()
        .any(|uid| {
            pool.entity(uid)
                .is_some_and(|entity| entity.battle_tags.contains(&BLOODTITHE_BATTLE_TAG))
        })
    {
        return None;
    }
    let origin = buff_act::feature_command_origin(enabler)?;
    Some(RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
        origin,
        key(team),
        GaugeOperation::Enable {
            max: Some(team_max_hp(managers, team)),
        },
    ))))
}

pub fn sync_base_max_rule_op(
    managers: &BattleManagers,
    enabler: &ActiveBuffFeature,
) -> Option<RuleOp> {
    let team = enabler.team_type;
    let gauge_key = key(team);
    let max = team_max_hp(managers, team);
    if managers.gauge.get(gauge_key).is_none() || managers.gauge.base_max(gauge_key) == Some(max) {
        return None;
    }
    let origin = buff_act::feature_command_origin(enabler)?;
    Some(RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
        origin,
        gauge_key,
        GaugeOperation::SyncBaseMax { max },
    ))))
}

fn team_max_hp(managers: &BattleManagers, team: i32) -> i32 {
    managers
        .buff
        .alive_team_uids(team, &managers.hp)
        .into_iter()
        .map(|uid| i64::from(managers.hp.max(uid)))
        .sum::<i64>()
        .saturating_div(1000)
        .clamp(0, i64::from(i32::MAX)) as i32
}

pub fn hp_loss_event_rule_op(
    managers: &BattleManagers,
    origin: CommandOrigin,
    team: i32,
    target_uid: i64,
    hp_loss: i32,
    skill_id: i32,
) -> Vec<RuleOp> {
    let key = key(team);
    let Some(state) = managers.gauge.get(key) else {
        return Vec::new();
    };
    let Some(max) = managers.gauge.accumulation_max(key) else {
        return Vec::new();
    };
    if hp_loss <= 0 || state.current >= max {
        return Vec::new();
    }
    let threshold = super::hp_loss_threshold(max);
    let command = GaugeCommand::new(
        origin,
        key,
        GaugeOperation::AccumulateValue {
            amount: hp_loss,
            threshold,
        },
    )
    .attributed_to(target_uid, 0)
    .caused_by_skill(skill_id);
    vec![RuleOp::Command(BattleCommand::Gauge(command))]
}

pub fn value_change_rule_ops(command: GaugeCommand) -> Vec<RuleOp> {
    vec![RuleOp::Command(BattleCommand::Gauge(command))]
}

pub fn faith_ex_point_rule_ops(
    managers: &BattleManagers,
    change: &GaugeChangeEvent,
) -> Vec<RuleOp> {
    let GaugeOwner::Team(team) = change.key.owner else {
        return Vec::new();
    };
    if change.key.kind != GaugeKind::Bloodtithe
        || !matches!(
            change.kind,
            GaugeChangeKind::Value | GaugeChangeKind::Accumulated
        )
        || change.applied_delta <= 0
    {
        return Vec::new();
    }
    managers
        .faith_ex_point_uids(team)
        .into_iter()
        .map(|target_uid| {
            RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
                ExPointChange {
                    origin: change.origin,
                    source_uid: change.source_uid,
                    target_uid,
                    delta: 1,
                    config_effect: 0,
                    effect_type: 0,
                },
            )))
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
}
