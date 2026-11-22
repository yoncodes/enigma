use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::card::{CardCommand, QueuedCardRankUp},
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::{ParsedBehavior, SkillEffectSlot},
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn supports_slot(slot: &SkillEffectSlot) -> bool {
        slot.compiled_subscriptions().is_ok_and(|keys| {
            keys.iter()
                .any(|key| key.event == EventKind::ActionQueueSubmitted)
        })
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [tag, slot] = behavior.args.as_slice() else {
            return None;
        };
        let BattleEvent::ActionQueueSubmitted { team, cards, .. } = context.event? else {
            return Some(Vec::new());
        };
        if *team != context.source_team {
            return Some(Vec::new());
        }

        let catalog = context.managers.catalog();
        let is_match = |skill_id| {
            catalog.skill_rank(skill_id) > 0
                && !catalog.skill_is_big(skill_id)
                && catalog.skill_effect_tag(skill_id) == *tag
        };
        let matching_count = cards
            .iter()
            .filter_map(|card| card.skill_id)
            .filter(|skill_id| is_match(*skill_id))
            .count();
        let levels = i32::try_from(matching_count.saturating_sub(1)).ok()?;
        if levels == 0 {
            return Some(Vec::new());
        }
        let upgrades: Vec<_> = context
            .managers
            .card
            .played()
            .iter()
            .filter(|played| {
                played.caster_uid == context.source_uid
                    && is_match(played.skill_id)
                    && context.pool.skill_slot(
                        context.managers,
                        context.source_uid,
                        played.skill_id,
                    ) == *slot
            })
            .map(|played| QueuedCardRankUp {
                card_index: played.card_index,
                levels,
            })
            .collect();
        if upgrades.is_empty() {
            return Some(Vec::new());
        }

        Some(vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::RewriteQueuedRanks {
                origin: super::command_origin(behavior)?,
                upgrades,
            },
        ))])
    }
}

pub(super) fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    behavior.raw_arity == 2 && matches!(behavior.args.as_slice(), [tag, 1 | 2] if *tag > 0)
}
