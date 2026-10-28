use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        card::{CardCommand, QueuedCardRankChange},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
    },
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [cost] = behavior.args.as_slice() else {
            return None;
        };
        if !matches!(cost, 1 | 2) {
            return None;
        }
        let BattleEvent::ActionQueueCommitted { team, .. } = context.event? else {
            return Some(Vec::new());
        };
        if *team != context.source_team {
            return Some(Vec::new());
        }

        let mut power = context
            .managers
            .eureka
            .get(context.source_uid, EUREKA_RESOURCE_ID)
            .current;
        let mut upgrades = Vec::new();
        for card in context
            .managers
            .card
            .played()
            .iter()
            .filter(|card| card.caster_uid == context.source_uid)
        {
            let levels = context
                .managers
                .card
                .available_played_rank_ups(card.card_index, power / cost);
            if levels == 0 {
                continue;
            }
            upgrades.push(QueuedCardRankChange {
                card_index: card.card_index,
                levels,
            });
            power -= levels * cost;
        }
        if upgrades.is_empty() {
            return Some(Vec::new());
        }

        let origin = super::command_origin(behavior)?;
        let consumed = context
            .managers
            .eureka
            .get(context.source_uid, EUREKA_RESOURCE_ID)
            .current
            - power;
        Some(vec![
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                origin,
                source_uid: context.source_uid,
                target_uid: context.source_uid,
                power_id: EUREKA_RESOURCE_ID,
                delta: -consumed,
                effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
            }))),
            RuleOp::Command(BattleCommand::Card(CardCommand::ChangeQueuedRanks {
                origin,
                changes: upgrades,
            })),
        ])
    }
}

pub(super) fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1 | 2])
}

#[cfg(test)]
mod tests;
