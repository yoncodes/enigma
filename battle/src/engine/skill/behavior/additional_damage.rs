use crate::engine::{
    manager::buff::{BuffCommand, BuffGrant},
    skill::{
        action::AdditionalDamageModifier,
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
        matches!(
            behavior.args.as_slice(),
            [count, chance, 2, buff_id]
                if *count > 0 && *chance > 0 && *buff_id > 0
        )
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [count, chance, _mode, buff_id] = behavior.args.as_slice() else {
            return None;
        };
        if *count <= 0
            || *chance <= 0
            || (*chance < 1000 && !context.determinism.roll_permille(*chance))
            || crate::engine::skill::buff_act::additional_damage::configured(
                context.managers.catalog(),
                *buff_id,
                context.source_uid,
                context.source_uid,
            )
            .is_none()
        {
            return None;
        }
        let modifier = AdditionalDamageModifier {
            origin: super::command_origin(behavior)?,
            buff_id: *buff_id,
        };
        context.modifiers.additional_damage.push(modifier);
        Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Grant(BuffGrant {
                origin: modifier.origin,
                source_uid: context.source_uid,
                target_uid: context.source_uid,
                buff_id: *buff_id,
                amount: None,
                occurrences: *count as u32,
                child_uid_reservations: 0,
            }),
        ))])
    }
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    RuleReferences {
        skills: Vec::new(),
        buffs: behavior.arg(3).into_iter().collect(),
        models: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::{self, classify::BehaviorSpec},
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn configured_additional_damage_is_cast_local_and_grants_its_data_buff() {
        crate::test_support::init_config();
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60206, "CreateAdditionalDamageAddBuff"),
            vec![1, 1000, 2, 31200111],
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31200114,
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
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    source_uid: 10,
                    target_uid: 10,
                    buff_id: 31200111,
                    ..
                }
            )))]
        ));
        assert_eq!(
            modifiers.additional_damage,
            vec![AdditionalDamageModifier {
                origin: super::super::command_origin(&behavior).unwrap(),
                buff_id: 31200111,
            }]
        );
    }
}
