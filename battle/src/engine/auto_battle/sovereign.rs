use std::collections::HashSet;

use sonettobuf::{CardInfo, card_info::CardType};

use crate::engine::skill::{behavior::classify::BehaviorKind, effect::SkillEffectCatalog};

use super::Candidate;

pub(super) mod vision;

fn is_precast(card: &CardInfo) -> bool {
    card.temp_card == Some(true) && card.card_type == Some(CardType::Skill3 as i32)
}

pub(super) fn is_charge(card: &CardInfo, skill_id: i32, catalog: &SkillEffectCatalog) -> bool {
    is_precast(card)
        && catalog.get(skill_id).is_some_and(|effect| {
            effect.slots.iter().any(|slot| {
                slot.behavior.spec.kind == BehaviorKind::HeatScaleUseSkillAddCount
                    && slot.behavior.arg(0).is_some_and(|amount| amount > 0)
            })
        })
}

pub(super) struct Order {
    charge_owners: HashSet<i64>,
    has_other_ultimate: bool,
}

impl Order {
    pub(super) fn new(candidates: &[Candidate]) -> Self {
        let charge_owners: HashSet<_> = candidates
            .iter()
            .filter(|candidate| candidate.sovereign_charge)
            .map(|candidate| candidate.source_uid)
            .collect();
        let has_other_ultimate = !charge_owners.is_empty()
            && candidates.iter().any(|candidate| {
                candidate.ultimate
                    && !candidate.free
                    && !charge_owners.contains(&candidate.source_uid)
            });
        Self {
            charge_owners,
            has_other_ultimate,
        }
    }

    pub(super) fn holds_charge(&self, candidate: &Candidate) -> bool {
        candidate.sovereign_charge && self.has_other_ultimate
    }

    pub(super) fn holds_ultimate(&self, candidate: &Candidate) -> bool {
        !candidate.free && self.charge_owners.contains(&candidate.source_uid)
    }
}
