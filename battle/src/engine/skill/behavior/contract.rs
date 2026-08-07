use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
        contract::ContractCommand,
    },
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::command_origin(behavior)?;
        let candidates = context
            .pool
            .main_allies(context.source_uid)
            .iter()
            .filter(|entity| entity.uid != context.source_uid)
            .map(|entity| entity.uid)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Some(Vec::new());
        }
        Some(vec![RuleOp::Command(BattleCommand::Contract(
            ContractCommand::Offer {
                origin,
                owner_uid: context.source_uid,
                candidates,
            },
        ))])
    }
}

pub(super) struct EndHandler;

impl BehaviorHandler for EndHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        related_buff_ids(behavior).is_some()
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            buffs: related_buff_ids(behavior)
                .map(|(owner, bound)| owner.into_iter().chain(bound).collect())
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let BattleEvent::EntityDied(death) = context.event? else {
            return Some(Vec::new());
        };
        let Some(bound_uid) = context.managers.contract.bound_uid(context.source_uid) else {
            return Some(Vec::new());
        };
        if death.target_uid != bound_uid {
            return Some(Vec::new());
        }
        let origin = super::command_origin(behavior)?;
        let (owner_buffs, bound_buffs) = related_buff_ids(behavior)?;
        let mut ops = owner_buffs
            .into_iter()
            .map(|buff_id| remove_buff(origin, context.source_uid, buff_id))
            .chain(
                bound_buffs
                    .into_iter()
                    .map(|buff_id| remove_buff(origin, bound_uid, buff_id)),
            )
            .collect::<Vec<_>>();
        ops.push(RuleOp::Command(BattleCommand::Contract(
            ContractCommand::Clear {
                owner_uid: context.source_uid,
                bound_uid,
            },
        )));
        Some(ops)
    }
}

fn related_buff_ids(behavior: &ParsedBehavior) -> Option<(Vec<i32>, Vec<i32>)> {
    let argument_count = if behavior.raw_args.is_empty() {
        behavior.args.len()
    } else {
        behavior.raw_args.len()
    };
    if argument_count != 2 {
        return None;
    }
    let owner = behavior.arg_list(0)?;
    let bound = behavior.arg_list(1)?;
    (!owner.is_empty()
        && !bound.is_empty()
        && owner.iter().chain(&bound).all(|buff_id| *buff_id > 0))
    .then_some((owner, bound))
}

fn remove_buff(
    origin: crate::engine::skill::rule::CommandOrigin,
    target_uid: i64,
    buff_id: i32,
) -> RuleOp {
    RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
        origin,
        target_uid,
        selector: BuffRemoveSelector::ExactId(buff_id),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::bus::EventBus,
        manager::BattleManagers,
        runtime::{
            determinism::RoundDeterminism,
            executor,
            record::{FrameItem, FrameOwner, FrameTrigger, SemanticFrame},
        },
        skill::{
            action::SkillModifiers,
            behavior::{self, classify::BehaviorSpec},
            target::{TargetContext, TargetPool},
        },
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    #[test]
    fn offer_projects_the_alive_other_allies_from_the_exact_behavior() {
        crate::test_support::init_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(-1),
                        current_hp: Some(100),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        current_hp: Some(100),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(30),
                        current_hp: Some(0),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60092, "NotifyHeroContract"),
            Vec::new(),
            Vec::new(),
        );
        let op = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: -1,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 31000142,
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
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
        let outcome =
            executor::execute_rule_op(&mut managers, &mut EventBus::default(), op).unwrap();
        assert!(managers.contract.selection_origin(-1, 20).is_some());
        assert!(managers.contract.selection_origin(-1, 30).is_none());

        let frame = SemanticFrame {
            owner: FrameOwner::EventRule,
            trigger: FrameTrigger::Active,
            items: outcome
                .changes()
                .into_iter()
                .map(|change| FrameItem::Change(Box::new(change)))
                .collect(),
        };
        let effect = crate::engine::packet::timeline::project_for_version(&[frame], 7)
            .unwrap()
            .into_iter()
            .flat_map(|step| step.act_effect)
            .next()
            .unwrap();
        assert_eq!(effect.target_id, Some(-1));
        assert_eq!(
            effect.effect_type,
            Some(sonettobuf::effect_type_enum::EffectType::Notifiyherocontract as i32)
        );
        assert_eq!(effect.config_effect, Some(60092));
        assert_eq!(effect.reserve_str.as_deref(), Some("20"));
    }

    #[test]
    fn bound_death_removes_both_configured_buff_groups_then_clears_the_pair() {
        let pool = TargetPool::default();
        let mut managers = BattleManagers::default();
        let origin = crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(60092, "NotifyHeroContract"),
        };
        managers
            .contract
            .execute(ContractCommand::Offer {
                origin,
                owner_uid: -1,
                candidates: vec![20],
            })
            .unwrap();
        managers
            .contract
            .execute(ContractCommand::SelectOwner {
                owner_uid: -1,
                bound_uid: 20,
            })
            .unwrap();
        managers
            .contract
            .execute(ContractCommand::SelectBound {
                owner_uid: -1,
                bound_uid: 20,
            })
            .unwrap();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60093, "ContractEndClearBuff"),
            Vec::new(),
            vec!["11,12".into(), "21,22".into()],
        );
        let event = BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 99,
            target_uid: 20,
        });
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: -1,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 0,
                transfer_count: 1,
                event: Some(&event),
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert_eq!(ops.len(), 5);
        for (op, target_uid, buff_id) in [
            (&ops[0], -1, 11),
            (&ops[1], -1, 12),
            (&ops[2], 20, 21),
            (&ops[3], 20, 22),
        ] {
            assert!(matches!(
                op,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    target_uid: actual_target,
                    selector: BuffRemoveSelector::ExactId(actual_buff),
                    ..
                }))) if *actual_target == target_uid && *actual_buff == buff_id
            ));
        }
        assert!(matches!(
            ops[4],
            RuleOp::Command(BattleCommand::Contract(ContractCommand::Clear {
                owner_uid: -1,
                bound_uid: 20,
            }))
        ));
    }
}
