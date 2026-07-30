use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant},
        gauge::{GaugeCommand, GaugeKey, GaugeKind, GaugeOperation},
    },
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        if matches!(
            behavior.spec.kind,
            BehaviorKind::ConsumeBloodAddBuff | BehaviorKind::ConsumeBloodAddBuff2
        ) {
            let spend = ConsumeBloodAddBuff::from_behavior(behavior)?;
            let gauge_key = crate::engine::mechanic::bloodtithe::rule::key(context.source_team);
            return Some(vec![RuleOp::Command(BattleCommand::BloodtitheSpend(
                crate::engine::mechanic::bloodtithe::spend::SpendCommand {
                    gauge: GaugeCommand::new(
                        origin,
                        gauge_key,
                        GaugeOperation::ChangeValue { delta: -spend.cost },
                    )
                    .attributed_to(context.target_uid, 0)
                    .caused_by_skill(context.active_skill_id),
                    buff: BuffCommand::Grant(BuffGrant {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        buff_id: spend.buff_id,
                        amount: spend.grant_amount(),
                        occurrences: 1,
                        child_uid_reservations: 0,
                    }),
                },
            ))]);
        }
        let mutation = SharedPoolMutation::from_behavior(behavior, context.source_team)?;
        let mut command = GaugeCommand::new(origin, mutation.key, mutation.operation)
            .attributed_to(
                context.source_uid,
                mutation.key.kind.shared_pool_config_effect(),
            )
            .caused_by_skill(context.active_skill_id);
        if let Some(raw_delta) = mutation.raw_delta {
            command = command.with_raw_delta(raw_delta);
        }
        if let Some(progress_raw_delta) = mutation.progress_raw_delta {
            command = command.with_progress_raw_delta(progress_raw_delta);
        }
        Some(route_shared_pool_change(context.managers, command))
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    RuleReferences {
        skills: Vec::new(),
        buffs: matches!(
            behavior.spec.kind,
            BehaviorKind::ConsumeBloodAddBuff | BehaviorKind::ConsumeBloodAddBuff2
        )
        .then(|| behavior.arg(1))
        .flatten()
        .into_iter()
        .collect(),
        models: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumeBloodAddBuff {
    pub cost: i32,
    pub buff_id: i32,
    pub count: i32,
}

impl ConsumeBloodAddBuff {
    pub fn from_behavior(behavior: &ParsedBehavior) -> Option<Self> {
        if !matches!(
            behavior.spec.kind,
            BehaviorKind::ConsumeBloodAddBuff | BehaviorKind::ConsumeBloodAddBuff2
        ) {
            return None;
        }

        let cost = behavior.arg(0)?;
        let buff_id = behavior.arg(1)?;
        let count = behavior.arg(2).unwrap_or(1);
        (cost > 0 && buff_id > 0 && count > 0).then_some(Self {
            cost,
            buff_id,
            count,
        })
    }

    fn grant_amount(self) -> Option<i32> {
        crate::engine::manager::buff::BuffManager::configured_accepts_explicit_grant_amount(
            self.buff_id,
        )
        .then_some(self.count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SharedPoolMutation {
    key: GaugeKey,
    operation: GaugeOperation,
    raw_delta: Option<i32>,
    progress_raw_delta: Option<i32>,
}

impl SharedPoolMutation {
    fn from_behavior(behavior: &ParsedBehavior, team: i32) -> Option<Self> {
        let raw_amount = behavior.arg(0)?;
        match behavior.spec.kind {
            BehaviorKind::BloodPoolMaxChange => Some(Self {
                key: crate::engine::mechanic::bloodtithe::rule::key(team),
                operation: GaugeOperation::ChangeMax { delta: raw_amount },
                raw_delta: None,
                progress_raw_delta: None,
            }),
            BehaviorKind::BloodPoolValueChange => {
                let kind =
                    GaugeKind::from_shared_pool_config_effect(behavior.arg(1).unwrap_or_default())?;
                let key = match kind {
                    GaugeKind::Bloodtithe => crate::engine::mechanic::bloodtithe::rule::key(team),
                    GaugeKind::LingeringGlow => crate::engine::mechanic::lingering_glow::key(team),
                    GaugeKind::TeamEnergy | GaugeKind::ImpromptuInspiration => return None,
                };
                let (delta, raw_delta) = match kind {
                    GaugeKind::LingeringGlow => (raw_amount / 1000, Some(raw_amount)),
                    GaugeKind::Bloodtithe => (raw_amount, None),
                    GaugeKind::TeamEnergy | GaugeKind::ImpromptuInspiration => return None,
                };
                Some(Self {
                    key,
                    operation: GaugeOperation::ChangeValue { delta },
                    raw_delta,
                    progress_raw_delta: raw_delta.filter(|_| kind == GaugeKind::LingeringGlow),
                })
            }
            _ => None,
        }
    }
}

pub(super) fn supports_shared_pool_mutation(behavior: &ParsedBehavior) -> bool {
    SharedPoolMutation::from_behavior(behavior, 1).is_some()
}

pub(super) fn supports_consume_blood_add_buff(behavior: &ParsedBehavior) -> bool {
    ConsumeBloodAddBuff::from_behavior(behavior).is_some()
}

fn route_shared_pool_change(
    managers: &crate::engine::manager::BattleManagers,
    command: GaugeCommand,
) -> Vec<RuleOp> {
    match command.key.kind {
        GaugeKind::Bloodtithe => {
            crate::engine::mechanic::bloodtithe::rule::value_change_rule_ops(command)
        }
        GaugeKind::LingeringGlow => {
            crate::engine::mechanic::lingering_glow::value_change_rule_ops(managers, command)
        }
        GaugeKind::TeamEnergy | GaugeKind::ImpromptuInspiration => {
            vec![RuleOp::Command(BattleCommand::Gauge(command))]
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        manager::{BattleManagers, buff::BuffCommand, gauge::GaugeOperation},
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::{BehaviorOpContext, classify::BehaviorSpec},
            effect::ParsedBehavior,
            rule::output::{BattleCommand, RuleOp},
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn parses_consume_blood_add_buff_rule_behavior() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60211, "ConsumeBloodAddBuff2"),
            vec![4, 31200143, 4],
            Vec::new(),
        );

        assert_eq!(
            super::ConsumeBloodAddBuff::from_behavior(&behavior),
            Some(super::ConsumeBloodAddBuff {
                cost: 4,
                buff_id: 31200143,
                count: 4,
            })
        );
        assert_eq!(super::references(&behavior).buffs, vec![31200143]);
    }

    #[test]
    fn count_argument_only_sets_supported_repeat_amount() {
        crate::test_support::init_config();

        assert_eq!(
            super::ConsumeBloodAddBuff {
                cost: 4,
                buff_id: 31200143,
                count: 4,
            }
            .grant_amount(),
            None
        );
        assert_eq!(
            super::ConsumeBloodAddBuff {
                cost: 1,
                buff_id: 31260151,
                count: 6,
            }
            .grant_amount(),
            Some(6)
        );
    }

    #[test]
    fn consume_blood_add_buff_emits_one_atomic_command() {
        crate::test_support::init_config();

        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60210, "ConsumeBloodAddBuff"),
            vec![1, 31260151, 6],
            Vec::new(),
        );
        let definition = super::super::registry::find(&behavior).unwrap();
        assert_eq!(
            definition.output_owner,
            super::super::registry::OutputOwner::Skill
        );
        let key = crate::engine::mechanic::bloodtithe::rule::key(1);
        let managers = BattleManagers::default();
        let emit = |managers: &BattleManagers| {
            let pool = TargetPool::default();
            let mut determinism = RoundDeterminism::default();
            let mut modifiers = SkillModifiers::default();
            let mut target = TargetContext::default();

            super::super::rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 20,
                    active_skill_id: 31260201,
                    transfer_count: 1,
                    event: None,
                    managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            )
            .unwrap()
        };

        let ops = emit(&managers);
        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::BloodtitheSpend(command))]
                if command.gauge.key == key
                && command.gauge.operation == GaugeOperation::ChangeValue { delta: -1 }
                && matches!(
                    &command.buff,
                    BuffCommand::Grant(grant)
                        if grant.source_uid == 10
                            && grant.target_uid == 20
                            && grant.buff_id == 31260151
                            && grant.amount == Some(6)
                )
        ));
    }

    #[test]
    fn lingering_glow_value_change_is_a_parent_owned_destination() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60191, "BloodPoolValueChange"),
            vec![20_000, 1],
            Vec::new(),
        );
        let definition = super::super::registry::find(&behavior).unwrap();

        assert!(definition.destination);
        assert_eq!(
            definition.output_owner,
            super::super::registry::OutputOwner::Parent
        );
        assert_eq!(
            super::SharedPoolMutation::from_behavior(&behavior, 1),
            Some(super::SharedPoolMutation {
                key: crate::engine::mechanic::lingering_glow::key(1),
                operation: crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 20 },
                raw_delta: Some(20_000),
                progress_raw_delta: Some(20_000),
            })
        );
    }

    #[test]
    fn ordinary_blood_pool_value_keeps_its_unscaled_parent_owned_points() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60191, "BloodPoolValueChange"),
            vec![20],
            Vec::new(),
        );

        assert_eq!(
            super::SharedPoolMutation::from_behavior(&behavior, 2),
            Some(super::SharedPoolMutation {
                key: crate::engine::mechanic::bloodtithe::rule::key(2),
                operation: crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 20 },
                raw_delta: None,
                progress_raw_delta: None,
            })
        );
        let definition = super::super::registry::find(&behavior).unwrap();
        assert_eq!((definition.output_owner_for)(&behavior, 0), None);
        assert_eq!(
            definition.output_owner,
            super::super::registry::OutputOwner::Parent
        );
    }
}
