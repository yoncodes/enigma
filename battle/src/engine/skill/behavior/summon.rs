use crate::engine::{
    manager::{
        buff::{BuffChildUidReservation, BuffCommand},
        entity::{EntityCommand, EntityOperation},
        summon::{SummonCommand, SummonOperation, summoned_lane},
    },
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub fn rule_op(source_uid: i64, target_uid: i64, behavior: &ParsedBehavior) -> Option<RuleOp> {
    let operation = match behavior.spec.kind {
        BehaviorKind::AddSummoned => SummonOperation::Add {
            target_uid,
            count: behavior.arg(1).unwrap_or(1),
            level: behavior.arg(2).unwrap_or(1),
        },
        BehaviorKind::ChangeSummonedLevel => SummonOperation::ChangeLevel {
            level: behavior.arg(1)?,
        },
        BehaviorKind::AddSummonedLevel => SummonOperation::AddLevel {
            delta: behavior.arg(1)?,
        },
        BehaviorKind::RemoveSummoned => SummonOperation::Remove { count: 0 },
        _ => return None,
    };
    Some(RuleOp::Command(BattleCommand::Summon(SummonCommand {
        origin: super::command_origin(behavior)?,
        owner_uid: source_uid,
        summoned_id: behavior.arg(0)?,
        operation,
    })))
}

pub(super) struct Handler;

pub(super) fn supports_combatant(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [model_id] if *model_id > 0
    ) || matches!(
        behavior.args.as_slice(),
        [model_id, position] if *model_id > 0 && (1..5).contains(position)
    )
}

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> crate::engine::skill::rule::RuleReferences {
        crate::engine::skill::rule::RuleReferences {
            models: matches!(
                behavior.spec.kind,
                BehaviorKind::Summon | BehaviorKind::SummonSp2
            )
            .then(|| behavior.arg(0))
            .flatten()
            .into_iter()
            .collect(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::Summon {
            let (model_id, position) = match behavior.args.as_slice() {
                [model_id] => (
                    *model_id,
                    context
                        .managers
                        .first_open_combat_position(context.source_uid),
                ),
                [model_id, position] => (*model_id, Some(*position)),
                _ => return None,
            };
            let Some(position) = position else {
                return Some(Vec::new());
            };
            return (model_id > 0).then(|| {
                vec![RuleOp::Command(BattleCommand::Entity(EntityCommand {
                    origin: super::command_origin(behavior).expect("registered behavior"),
                    source_uid: context.source_uid,
                    target_uid: context.source_uid,
                    operation: EntityOperation::SummonCombatant { model_id, position },
                }))]
            });
        }
        if behavior.spec.kind == BehaviorKind::SummonSp2 {
            let [model_id] = behavior.args.as_slice() else {
                return None;
            };
            return (*model_id > 0).then(|| {
                vec![RuleOp::Command(BattleCommand::Entity(EntityCommand {
                    origin: super::command_origin(behavior).expect("registered behavior"),
                    source_uid: context.source_uid,
                    target_uid: context.source_uid,
                    operation: EntityOperation::SummonSpecial {
                        model_id: *model_id,
                    },
                }))]
            });
        }
        let summon = rule_op(context.source_uid, context.target_uid, behavior)?;
        let mut ops = vec![summon];
        if behavior.spec.kind == BehaviorKind::AddSummoned {
            let count = summoned_lane(behavior.arg(0)?);
            if count > 0 {
                ops.push(RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::ReserveChildUids(BuffChildUidReservation {
                        origin: super::command_origin(behavior)?,
                        target_uid: context.source_uid,
                        count,
                    }),
                )));
            }
        }
        Some(ops)
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::{classify::BehaviorSpec, registry},
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn positioned_summon_is_registry_ready() {
        let behavior = ParsedBehavior::new(60008, "Summon", vec![150402, 2]);
        let definition = registry::find(&behavior).unwrap();

        assert!(
            definition
                .supports
                .is_some_and(|supports| supports(&behavior))
        );
    }

    #[test]
    fn summon_sp2_emits_entity_state_command_from_configured_model() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60056, "SummonSp2"),
            vec![151416],
            vec!["151416".into()],
        );

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: -2,
                source_team: 2,
                target_uid: -2,
                active_skill_id: 0,
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
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Entity(EntityCommand {
                source_uid: -2,
                target_uid: -2,
                operation: EntityOperation::SummonSpecial { model_id: 151416 },
                ..
            }))]
        ));
    }

    #[test]
    fn summon_uses_the_configured_or_first_open_combat_position() {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    position: Some(1),
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
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        for (args, raw_args, expected_position) in [
            (vec![30111003], vec!["30111003".into()], 2),
            (vec![30111003, 3], vec!["30111003".into(), "3".into()], 3),
        ] {
            let behavior =
                ParsedBehavior::from_spec(BehaviorSpec::new(60008, "Summon"), args, raw_args);
            let ops = Handler::emit_ops(
                BehaviorOpContext {
                    source_uid: -1,
                    source_team: 2,
                    target_uid: -1,
                    active_skill_id: 530002744,
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
                ops.as_slice(),
                [RuleOp::Command(BattleCommand::Entity(EntityCommand {
                    source_uid: -1,
                    target_uid: -1,
                    operation: EntityOperation::SummonCombatant {
                        model_id: 30111003,
                        position,
                    },
                    ..
                }))] if *position == expected_position
            ));
        }
    }

    #[test]
    fn summon_is_a_valid_noop_when_all_combat_positions_are_occupied() {
        crate::test_support::init_config();
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: (1..=4)
                    .map(|position| FightEntityInfo {
                        uid: Some(-i64::from(position)),
                        team_type: Some(2),
                        position: Some(position),
                        current_hp: Some(100),
                        attr: Some(HeroAttribute {
                            hp: Some(100),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60008, "Summon"),
            vec![900110102],
            vec!["900110102".into()],
        );

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: -1,
                source_team: 2,
                target_uid: -1,
                active_skill_id: 23390171,
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

        assert!(ops.is_empty());
    }
}
