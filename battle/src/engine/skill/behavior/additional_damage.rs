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
        summons: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
