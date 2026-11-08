use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant},
        card::{CardAddHedone, CardCommand},
    },
    mechanic::hedone_starting_charge::HedoneStartingChargeCommand,
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

fn args(behavior: &ParsedBehavior) -> Option<(i32, i32, Option<i32>)> {
    match behavior.args.as_slice() {
        [skill_id, buff_id] if *skill_id > 0 && *buff_id > 0 => Some((*skill_id, *buff_id, None)),
        [skill_id, buff_id, amount] if *skill_id > 0 && *buff_id > 0 && *amount > 0 => {
            Some((*skill_id, *buff_id, Some(*amount)))
        }
        _ => None,
    }
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        args(behavior).is_some()
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        let Some((skill_id, buff_id, _)) = args(behavior) else {
            return RuleReferences::default();
        };
        RuleReferences {
            skills: vec![skill_id],
            buffs: vec![buff_id],
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let (skill_id, buff_id, starting_charge) = args(behavior)?;
        if context
            .managers
            .card
            .has_hedone_card(context.target_uid, skill_id)
        {
            return Some(Vec::new());
        }
        let hero_id = context.pool.entity(context.target_uid)?.model_id;
        let team_type = context.pool.team_type(context.target_uid)?;
        let origin = super::command_origin(behavior)?;
        let mut ops = vec![
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::AddHedone(CardAddHedone {
                origin,
                owner_uid: context.target_uid,
                skill_id,
                hero_id,
                team_type,
            }))),
        ];
        if let Some(amount) = starting_charge {
            ops.push(RuleOp::Command(BattleCommand::HedoneStartingCharge(
                HedoneStartingChargeCommand {
                    origin,
                    owner_uid: context.target_uid,
                    amount,
                },
            )));
        }
        Some(ops)
    }
}

#[cfg(test)]
mod tests;
