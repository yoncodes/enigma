use crate::engine::{
    manager::card::{CardAddCrystal, CardCommand},
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
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let groups = candidate_groups(behavior);
        let candidates = groups.iter().flatten().copied().collect::<Vec<_>>();
        if candidates.is_empty() {
            return Some(Vec::new());
        }
        let captured = context.determinism.take_crystal_card(&candidates);
        let weights = rank_weights(behavior);
        let rank_roll = context.determinism.condition_random_roll(
            context.active_skill_id,
            context.source_uid,
            behavior.spec.key.opcode,
        );
        let crystal_roll = context.determinism.condition_random_roll(
            context.active_skill_id,
            context.source_uid,
            behavior.spec.key.opcode,
        );
        let skill_id = captured
            .as_ref()
            .and_then(|card| card.skill_id)
            .or_else(|| {
                let rank = weighted_index(&weights, rank_roll).min(groups.len().saturating_sub(1));
                let crystal = context
                    .managers
                    .emanation
                    .choose(context.source_uid, crystal_roll)?;
                groups.get(rank)?.get(crystal).copied()
            });
        let Some(skill_id) = skill_id else {
            return Some(Vec::new());
        };
        let crystal = groups
            .iter()
            .find_map(|group| group.iter().position(|candidate| *candidate == skill_id))?;
        let rank_group = groups
            .iter()
            .filter_map(|group| group.get(crystal).copied())
            .collect();
        let card = generated_card(context.managers, context.source_uid, skill_id);
        let origin = super::command_origin(behavior)?;
        Some(vec![RuleOp::Command(BattleCommand::Card(
            CardCommand::AddCrystal(CardAddCrystal {
                origin,
                card,
                rank_group,
            }),
        ))])
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: candidate_groups(behavior).into_iter().flatten().collect(),
            buffs: Vec::new(),
            models: Vec::new(),
            summons: Vec::new(),
        }
    }
}

fn rank_weights(behavior: &ParsedBehavior) -> Vec<i32> {
    behavior
        .raw_args
        .iter()
        .take(3)
        .map(|raw| {
            raw.split(',')
                .next()
                .and_then(|value| value.trim().parse::<i32>().ok())
                .unwrap_or_default()
                .max(0)
        })
        .collect()
}

fn weighted_index(weights: &[i32], roll: i32) -> usize {
    let total = weights.iter().sum::<i32>().max(1);
    let mut point = roll.clamp(0, 999) * total / 1000;
    for (index, weight) in weights.iter().enumerate() {
        if point < *weight {
            return index;
        }
        point -= *weight;
    }
    weights.len().saturating_sub(1)
}

fn generated_card(
    managers: &crate::engine::manager::BattleManagers,
    source_uid: i64,
    skill_id: i32,
) -> sonettobuf::CardInfo {
    crate::engine::manager::card::temp::runtime_precast_card(managers, source_uid, skill_id)
}

fn candidate_groups(behavior: &ParsedBehavior) -> Vec<Vec<i32>> {
    behavior
        .raw_args
        .iter()
        .skip(3)
        .map(|raw| {
            raw.split(',')
                .filter_map(|value| value.trim().parse().ok())
                .collect()
        })
        .filter(|group: &Vec<i32>| !group.is_empty())
        .collect()
}

pub(super) fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    rank_weights(behavior).len() == 3 && !candidate_groups(behavior).is_empty()
}

#[cfg(test)]
mod tests;
