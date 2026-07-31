use crate::engine::skill::{
    behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
    effect::ParsedBehavior,
    rule::{
        RuleReferences,
        output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

pub(super) fn supports_add_buff_and_count(behavior: &ParsedBehavior) -> bool {
    matches!(
        (
            behavior.arg(0),
            behavior.arg(1),
            behavior.arg(2),
            behavior.arg_list(3)
        ),
        (Some(buff_id), Some(buff_count), Some(special_count), Some(marker_ids))
            if buff_id > 0
                && buff_count > 0
                && special_count > 0
                && (marker_ids == [-1] || marker_ids.iter().all(|marker_id| *marker_id > 0))
    )
}

pub(super) fn supports_add_count(behavior: &ParsedBehavior) -> bool {
    matches!(
        (behavior.arg(0), behavior.arg_list(1)),
        (Some(count), Some(marker_ids))
            if count > 0 && marker_ids.iter().all(|marker_id| *marker_id > 0)
    )
}

pub(super) fn supports_rate(behavior: &ParsedBehavior) -> bool {
    matches!(
        (behavior.arg(0), behavior.arg_list(1)),
        (Some(rate), Some(marker_ids))
            if rate != 0 && marker_ids.iter().all(|marker_id| *marker_id > 0)
    )
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        match behavior.spec.kind {
            BehaviorKind::AddBuffAndAddSpecialCount => {
                let buff_id = behavior.arg(0)?;
                let buff_count = behavior.arg(1)?;
                let special_count = behavior
                    .arg(2)?
                    .max(0)
                    .saturating_mul(context.transfer_count);
                let marker_ids = behavior
                    .arg_list(3)?
                    .into_iter()
                    .filter(|id| *id > 0)
                    .collect::<Vec<_>>();
                let mut ops = Vec::new();
                if buff_id > 0 && buff_count > 0 {
                    ops.push(RuleOp::Command(BattleCommand::Buff(
                        crate::engine::manager::buff::BuffCommand::Grant(
                            crate::engine::manager::buff::BuffGrant {
                                origin,
                                source_uid: context.source_uid,
                                target_uid: context.target_uid,
                                buff_id,
                                amount: Some(buff_count.saturating_mul(context.transfer_count)),
                                occurrences: 1,
                                child_uid_reservations: if marker_ids.is_empty() {
                                    special_count as u32
                                } else {
                                    0
                                },
                            },
                        ),
                    )));
                }
                if special_count > 0 && !marker_ids.is_empty() {
                    ops.push(RuleOp::Command(BattleCommand::Buff(
                        crate::engine::manager::buff::BuffCommand::AddSpecialCount(
                            crate::engine::manager::buff::BuffSpecialCount {
                                origin,
                                target_uid: context.target_uid,
                                marker_ids,
                                count: special_count,
                            },
                        ),
                    )));
                }
                Some(ops)
            }
            BehaviorKind::AddBuffSpecialCount => {
                let transfer_unit = behavior.arg(0)?.max(0);
                let marker_ids = behavior
                    .arg_list(1)?
                    .into_iter()
                    .filter(|id| *id > 0)
                    .collect::<Vec<_>>();
                let count = transfer_unit.saturating_mul(context.transfer_count);
                if count <= 0 || marker_ids.is_empty() {
                    return None;
                }
                Some(vec![RuleOp::Command(BattleCommand::Buff(
                    crate::engine::manager::buff::BuffCommand::AddSpecialCount(
                        crate::engine::manager::buff::BuffSpecialCount {
                            origin,
                            target_uid: context.target_uid,
                            marker_ids,
                            count,
                        },
                    ),
                ))])
            }
            BehaviorKind::AddSkillRateBySpecialCount => {
                let rate = behavior.arg(0)?;
                let marker_ids = behavior.arg_list(1)?;
                let count = context
                    .managers
                    .buff
                    .special_count(context.source_uid, &marker_ids);
                if context.active_skill_id != 0 && rate != 0 && count > 0 {
                    context.modifiers.rates.push(
                        crate::engine::skill::action::SkillRateModifier::fixed(
                            0,
                            behavior.spec.key.opcode,
                            rate * count,
                            true,
                        ),
                    );
                }
                Some(Vec::new())
            }
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }

    fn resolve_fire_count(
        context: super::registry::BehaviorFireCountContext<'_>,
        behavior: &ParsedBehavior,
        fallback: i32,
    ) -> i32 {
        if behavior.spec.kind == BehaviorKind::AddBuffAndAddSpecialCount {
            context
                .managers
                .field
                .round_transfer_count(context.source_team)
        } else {
            fallback
        }
    }
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    RuleReferences {
        skills: Vec::new(),
        buffs: (behavior.spec.kind == BehaviorKind::AddBuffAndAddSpecialCount)
            .then(|| behavior.arg(0))
            .flatten()
            .into_iter()
            .collect(),
        models: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        skill::{
            action::SkillModifiers,
            rule::output::BattleCommand,
            target::{TargetContext, TargetPool},
        },
    };
    use crate::test_support::init_config;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    #[test]
    fn descriptor_reports_added_buff() {
        let behavior =
            ParsedBehavior::new(60205, "AddBuffAndAddSpecialCount", vec![101, 1, 1, 102]);

        assert_eq!(references(&behavior).buffs, vec![101]);
    }

    #[test]
    fn special_count_rate_is_emitted_as_a_skill_modifier() {
        init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(90071),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers.buff.add_special_count(10, &[90071], 3);
        let pool = TargetPool::default();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let parsed = ParsedBehavior::from_spec(
            ParsedBehavior::new(60202, "AddSkillRateBySpecialCount", vec![250, 90071]).spec,
            vec![250, 90071],
            vec!["250".into(), "90071".into()],
        );

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 123,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &parsed,
        );

        assert_eq!(ops, Some(Vec::new()));
        assert_eq!(modifiers.rates.len(), 1);
        assert_eq!(modifiers.rates[0].fixed_value(), Some(750));
        assert_eq!(modifiers.rates[0].target_uid, 0);
    }

    #[test]
    fn combined_special_count_scales_configured_values_by_transfer_count() {
        init_config();
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(
            60205,
            "AddBuffAndAddSpecialCount",
            vec![90071, 5, 1, 31070111],
        );

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 123,
                transfer_count: 3,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            &ops[0],
            RuleOp::Command(BattleCommand::Buff(
                crate::engine::manager::buff::BuffCommand::Grant(grant)
            )) if grant.amount == Some(15)
        ));
        assert!(matches!(
            &ops[1],
            RuleOp::Command(BattleCommand::Buff(
                crate::engine::manager::buff::BuffCommand::AddSpecialCount(change)
            )) if change.count == 3
        ));
    }

    #[test]
    fn field_transfer_count_is_resolved_through_the_registered_behavior() {
        let mut managers = BattleManagers::default();
        managers.field.record_transfer(1);
        managers.field.record_transfer(1);
        let behavior =
            ParsedBehavior::new(60205, "AddBuffAndAddSpecialCount", vec![435021, 1, 1, -1]);
        let definition = super::super::registry::find(&behavior).unwrap();
        assert_eq!(
            (definition.resolve_fire_count)(
                super::super::registry::BehaviorFireCountContext {
                    managers: &managers,
                    source_team: 1,
                },
                &behavior,
                1,
            ),
            2
        );
    }

    #[test]
    fn configured_special_count_transfer_unit_is_not_collapsed_to_one() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(60204, "AddBuffSpecialCount", vec![5, 31070111]);

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 123,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            &ops[0],
            RuleOp::Command(BattleCommand::Buff(
                crate::engine::manager::buff::BuffCommand::AddSpecialCount(change)
            )) if change.count == 5
        ));
    }
}
