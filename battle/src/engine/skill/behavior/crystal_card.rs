use crate::engine::{
    manager::{
        buff::{BuffChildUidReservation, BuffCommand},
        card::{CardAddCrystal, CardCommand},
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
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let groups = candidate_groups(behavior);
        let candidates = groups.iter().flatten().copied().collect::<Vec<_>>();
        if candidates.is_empty() {
            return Some(Vec::new());
        }
        let captured = context.determinism.take_crystal_card(&candidates);
        let weights = rank_weights(behavior);
        let rank_roll = context
            .determinism
            .condition_random_roll(context.active_skill_id, behavior.spec.key.opcode);
        let crystal_roll = context
            .determinism
            .condition_random_roll(context.active_skill_id, behavior.spec.key.opcode);
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
        let card = generated_card(context.source_uid, skill_id);
        let origin = super::command_origin(behavior)?;
        Some(vec![
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveChildUids(
                BuffChildUidReservation {
                    origin,
                    target_uid: context.source_uid,
                    count: 2,
                },
            ))),
            RuleOp::Command(BattleCommand::Card(CardCommand::AddCrystal(
                CardAddCrystal {
                    origin,
                    card,
                    rank_group,
                },
            ))),
        ])
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: candidate_groups(behavior).into_iter().flatten().collect(),
            buffs: Vec::new(),
            models: Vec::new(),
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

fn generated_card(source_uid: i64, skill_id: i32) -> sonettobuf::CardInfo {
    crate::engine::manager::card::precast_card(source_uid, skill_id)
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
mod tests {
    use sonettobuf::CardInfo;

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::classify::BehaviorSpec,
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn generated_crystal_is_an_owner_bound_precast_card() {
        let card = generated_card(10, 101);

        assert_eq!(card.uid, Some(10));
        assert_eq!(card.temp_card, Some(true));
        assert_eq!(
            card.card_type,
            Some(sonettobuf::card_info::CardType::Skill3 as i32)
        );
    }

    #[test]
    fn crystal_card_reserves_its_two_internal_buff_uids() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_crystal_cards([CardInfo {
            uid: Some(999),
            skill_id: Some(102),
            temp_card: Some(false),
            card_type: Some(123),
            ..Default::default()
        }]);
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60245, "CrystalAddCard"),
            Vec::new(),
            vec![
                "400,-100".into(),
                "500,0".into(),
                "100,100".into(),
                "101,201".into(),
                "102,202".into(),
                "103,203".into(),
            ],
        );

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 1,
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
            &ops[0],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveChildUids(
                BuffChildUidReservation {
                    target_uid: 10,
                    count: 2,
                    ..
                }
            )))
        ));
        assert!(matches!(
            &ops[1],
            RuleOp::Command(BattleCommand::Card(CardCommand::AddCrystal(add)))
                if add.rank_group == vec![101, 102, 103]
                    && add.card.uid == Some(10)
                    && add.card.skill_id == Some(102)
                    && add.card.temp_card == Some(true)
                    && add.card.card_type
                        == Some(sonettobuf::card_info::CardType::Skill3 as i32)
        ));
    }
}
