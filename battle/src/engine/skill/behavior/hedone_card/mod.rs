use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant},
        card::{CardAddHedone, CardCommand},
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

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [skill_id, buff_id] if *skill_id > 0 && *buff_id > 0)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        let [skill_id, buff_id] = behavior.args.as_slice() else {
            return RuleReferences::default();
        };
        RuleReferences {
            skills: vec![*skill_id],
            buffs: vec![*buff_id],
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [skill_id, buff_id] = behavior.args.as_slice() else {
            return None;
        };
        if context
            .managers
            .card
            .has_hedone_card(context.target_uid, *skill_id)
        {
            return Some(Vec::new());
        }
        let hero_id = context.pool.entity(context.target_uid)?.model_id;
        let team_type = context.pool.team_type(context.target_uid)?;
        let origin = super::command_origin(behavior)?;
        Some(vec![
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id: *buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::AddHedone(CardAddHedone {
                origin,
                owner_uid: context.target_uid,
                skill_id: *skill_id,
                hero_id,
                team_type,
            }))),
        ])
    }
}

#[cfg(test)]
mod tests;
