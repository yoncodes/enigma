use crate::engine::{
    manager::contract::ContractCommand,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
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
}
