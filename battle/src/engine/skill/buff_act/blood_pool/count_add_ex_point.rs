use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        ex_point::{ExPointChange, ExPointChanges, ExPointCommand, ExPointCommandError},
        gauge::GaugeKey,
    },
    skill::{
        action::SkillPhase,
        buff_act::{self, is_kind, registry::BuffActKind},
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloodPoolCountAddExPointFeature {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub team_type: i32,
    pub act_id: i32,
    pub threshold: i32,
    pub amount: i32,
}

impl BloodPoolCountAddExPointFeature {
    pub fn from_feature(feature: &ActiveBuffFeature) -> Option<Self> {
        if !is_kind(feature, BuffActKind::BloodPoolCountAddExPoint) {
            return None;
        }
        let [act_id, threshold, amount] = feature.values.as_slice() else {
            return None;
        };
        (*threshold > 0 && *amount > 0).then_some(Self {
            owner_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            buff_id: feature.buff_id,
            team_type: feature.team_type,
            act_id: *act_id,
            threshold: *threshold,
            amount: *amount,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloodPoolCountAddExPointCommand {
    pub origin: CommandOrigin,
    pub key: GaugeKey,
    pub listener_uid: i64,
    pub listener_opcode: i32,
    pub source_uid: i64,
    pub target_uid: i64,
    pub threshold: i32,
    pub amount: i32,
}

fn command(
    active: &ActiveBuffFeature,
    feature: BloodPoolCountAddExPointFeature,
    key: GaugeKey,
) -> Option<RuleOp> {
    Some(RuleOp::Command(BattleCommand::BloodPoolCountAddExPoint(
        BloodPoolCountAddExPointCommand {
            origin: buff_act::feature_command_origin(active)?,
            key,
            listener_uid: feature.buff_uid,
            listener_opcode: feature.act_id,
            source_uid: feature.owner_uid,
            target_uid: feature.owner_uid,
            threshold: feature.threshold,
            amount: feature.amount,
        },
    )))
}

pub fn event_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::SkillAction(action) = event else {
        return Vec::new();
    };
    if action.phase != SkillPhase::AfterHit {
        return Vec::new();
    }

    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter_map(|active| {
            let feature = BloodPoolCountAddExPointFeature::from_feature(&active)?;
            let key = crate::engine::mechanic::bloodtithe::rule::key(feature.team_type);
            (managers.gauge.preview_positive_threshold(
                key,
                feature.buff_uid,
                feature.act_id,
                feature.threshold,
                feature.amount,
            ) > 0)
                .then(|| command(&active, feature, key).map(|op| (active, op)))?
        })
        .collect()
}

pub fn setup_rule_ops(
    managers: &BattleManagers,
    active: &ActiveBuffFeature,
) -> Option<Vec<RuleOp>> {
    let feature = BloodPoolCountAddExPointFeature::from_feature(active)?;
    let key = crate::engine::mechanic::bloodtithe::rule::key(feature.team_type);
    if managers.gauge.preview_positive_threshold(
        key,
        feature.buff_uid,
        feature.act_id,
        feature.threshold,
        feature.amount,
    ) <= 0
    {
        return Some(Vec::new());
    }
    Some(
        super::super::with_feature_runtime_markers(vec![(
            active.clone(),
            command(active, feature, key)?,
        )])
        .into_iter()
        .map(|(_, op)| op)
        .collect(),
    )
}

pub fn execute(
    managers: &mut BattleManagers,
    command: BloodPoolCountAddExPointCommand,
) -> Result<Option<ExPointChanges>, ExPointCommandError> {
    let delta = managers.gauge.settle_positive_threshold(
        command.key,
        command.listener_uid,
        command.listener_opcode,
        command.threshold,
        command.amount,
    );
    if delta <= 0 {
        return Ok(None);
    }
    managers
        .execute_ex_point(ExPointCommand::Change(ExPointChange {
            origin: command.origin,
            source_uid: command.source_uid,
            target_uid: command.target_uid,
            delta,
            config_effect: 0,
            effect_type: 0,
        }))
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::{
            buff::BuffGrant,
            gauge::{GaugeCommand, GaugeOperation},
        },
        skill::rule::{DefinitionKey, RuleDomain},
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    #[test]
    fn round_start_setup_queues_the_configured_threshold_rule() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
                BuffGrant {
                    origin: origin(),
                    source_uid: 20,
                    target_uid: 20,
                    buff_id: 308802111,
                    amount: Some(1),
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap();
        let key = crate::engine::mechanic::bloodtithe::rule::key(1);

        managers
            .execute_gauge(GaugeCommand::new(
                origin(),
                key,
                GaugeOperation::Enable { max: Some(56) },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin(),
                key,
                GaugeOperation::ChangeValue { delta: 8 },
            ))
            .unwrap();
        let active = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| feature.buff_id == 308802111)
            .unwrap();
        let ops = setup_rule_ops(&managers, &active).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::BuffFeatureMarker {
                    effect_num: 308802111,
                    buff_act_id: 1021,
                    ..
                },
                RuleOp::Command(BattleCommand::BloodPoolCountAddExPoint(
                    BloodPoolCountAddExPointCommand {
                        threshold: 8,
                        amount: 1,
                        ..
                    }
                ))
            ]
        ));
    }

    #[test]
    fn event_does_not_emit_a_marker_before_the_threshold_crosses() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
                BuffGrant {
                    origin: origin(),
                    source_uid: 20,
                    target_uid: 20,
                    buff_id: 308802111,
                    amount: Some(1),
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap();
        let key = crate::engine::mechanic::bloodtithe::rule::key(1);
        managers
            .execute_gauge(GaugeCommand::new(
                origin(),
                key,
                GaugeOperation::Enable { max: Some(56) },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin(),
                key,
                GaugeOperation::ChangeValue { delta: 7 },
            ))
            .unwrap();
        let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            source_uid: 20,
            skill_id: 1,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: SkillPhase::AfterHit,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 0,
            assassinate: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        });

        assert!(event_rule_ops(&managers, &event).is_empty());
    }

    #[test]
    fn execution_uses_cumulative_positive_gain_after_the_gauge_is_consumed() {
        let mut managers = BattleManagers::default();
        let key = crate::engine::mechanic::bloodtithe::rule::key(1);
        managers
            .execute_gauge(GaugeCommand::new(
                origin(),
                key,
                GaugeOperation::Enable { max: Some(56) },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin(),
                key,
                GaugeOperation::ChangeValue { delta: 8 },
            ))
            .unwrap();
        managers.ex_point.set(20, 20, 0, 0);
        let command = BloodPoolCountAddExPointCommand {
            origin: origin(),
            key,
            listener_uid: 1,
            listener_opcode: 1021,
            source_uid: 20,
            target_uid: 20,
            threshold: 8,
            amount: 1,
        };

        assert!(execute(&mut managers, command).unwrap().is_some());
        assert_eq!(managers.ex_point.get(20), 1);
        assert!(execute(&mut managers, command).unwrap().is_none());
    }

    fn origin() -> CommandOrigin {
        CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(1021, "BloodPoolCountAddExPoint"),
        }
    }
}
