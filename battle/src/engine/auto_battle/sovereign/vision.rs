use std::collections::HashSet;

use sonettobuf::CardInfo;

use crate::engine::{
    manager::{BattleManagers, emanation::EmanationKind},
    skill::{behavior::crystal_card, effect::SkillEffectCatalog},
};

use super::{Candidate, is_precast};

pub(in crate::engine::auto_battle) fn is_vision(
    card: &CardInfo,
    owner_uid: i64,
    skill_id: i32,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
) -> bool {
    is_precast(card)
        && crystal_card::has_kind(
            managers,
            catalog,
            owner_uid,
            skill_id,
            EmanationKind::Purple,
        )
}

#[derive(Default)]
pub(in crate::engine::auto_battle) struct Order {
    paid_ultimate_owners: HashSet<i64>,
}

impl Order {
    pub(in crate::engine::auto_battle) fn record(&mut self, candidate: &Candidate) {
        if candidate.ultimate && !candidate.free {
            self.paid_ultimate_owners.insert(candidate.source_uid);
        }
    }

    pub(in crate::engine::auto_battle) fn waits_for_ultimate(
        &self,
        candidate: &Candidate,
        candidates: &[Candidate],
    ) -> bool {
        candidate.sovereign_vision
            && !self
                .paid_ultimate_owners
                .iter()
                .any(|&owner| owner != candidate.source_uid)
            && candidates.iter().any(|other| {
                other.ultimate && !other.free && other.source_uid != candidate.source_uid
            })
    }
}
