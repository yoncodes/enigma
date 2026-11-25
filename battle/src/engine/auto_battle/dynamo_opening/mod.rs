use sonettobuf::Fight;

use crate::engine::{
    manager::card::CardManager,
    skill::{
        behavior::classify::BehaviorKind,
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::request::MAIN_ALLIES_TARGET_CODE,
    },
};

use super::{Candidate, Support};

pub(super) enum Order {
    Inactive,
    Pending,
    Active,
}

impl Order {
    pub(super) fn new(fight: &Fight, ordinary_queue: bool) -> Self {
        if fight.version == Some(7) && fight.cur_round == Some(1) && ordinary_queue {
            Self::Pending
        } else {
            Self::Inactive
        }
    }

    pub(super) fn pick<'a>(
        &mut self,
        candidates: &'a [Candidate],
        best: Option<&Candidate>,
        cards: &CardManager,
        catalog: &SkillEffectCatalog,
    ) -> Option<&'a Candidate> {
        let preparations = || {
            candidates.iter().filter(|candidate| {
                ordinary_rank_one(candidate, cards)
                    && candidate.strength == Some((0, 1))
                    && candidate.support != Support::Unneeded
                    && candidate.chosen_skill_id.is_none()
            })
        };
        if matches!(self, Self::Pending) {
            *self = if preparations().any(|candidate| channel(candidate.skill_id, catalog)) {
                Self::Active
            } else {
                Self::Inactive
            };
        }
        let best = best?;
        if !matches!(self, Self::Active)
            || !ordinary_rank_one(best, cards)
            || best.support >= Support::Needed
        {
            return None;
        }
        preparations()
            .filter(|candidate| {
                channel(candidate.skill_id, catalog) || team_buff(candidate.skill_id, catalog)
            })
            .min_by_key(|candidate| candidate.card_index)
    }
}

fn ordinary_rank_one(candidate: &Candidate, cards: &CardManager) -> bool {
    !candidate.free
        && candidate.strength.is_some_and(|(_, rank)| rank == 1)
        && cards
            .visible_card(candidate.card_index)
            .is_some_and(|card| {
                !card.temp_card.unwrap_or_default() && card.energy.unwrap_or_default() == 0
            })
}

fn channel(skill_id: i32, catalog: &SkillEffectCatalog) -> bool {
    catalog.effect_tag(skill_id) == SkillEffectTag::Channel as i32
        && catalog.get(skill_id).is_some_and(|effect| {
            effect
                .slots
                .iter()
                .any(|slot| slot.behavior.spec.kind == BehaviorKind::ElectricTransform)
        })
}

fn team_buff(skill_id: i32, catalog: &SkillEffectCatalog) -> bool {
    catalog.effect_tag(skill_id) == SkillEffectTag::Buff as i32
        && catalog.logic_target(skill_id) == MAIN_ALLIES_TARGET_CODE
}
