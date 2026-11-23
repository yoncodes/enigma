use sonettobuf::{FightStep, UseClothSkillRequest};

use crate::engine::{
    manager::card::CardStateSnapshot,
    mechanic::crystal_selection,
    runtime::{
        BattleRuntime,
        change::BattleChange,
        record::{FrameItem, FrameOwner, FrameTrigger, SemanticFrame},
    },
};

impl BattleRuntime {
    pub(super) fn select_crystal(
        &mut self,
        request: &UseClothSkillRequest,
    ) -> Option<Vec<FightStep>> {
        let owner_uid = request.from_id?;
        let packed = i32::try_from(request.to_id?).ok()?;
        let (choice, marker) = crystal_selection::select(&mut self.managers, owner_uid, packed)?;
        let cards = &self.managers.card;
        let hand = CardStateSnapshot {
            hand: cards.visible_cards().cloned().collect(),
            deck_num: cards.deck_num(),
            team_type: choice.team_type,
        };
        let frames = [
            SemanticFrame {
                owner: FrameOwner::EventEffect {
                    source_uid: owner_uid,
                    target_uid: 0,
                },
                trigger: FrameTrigger::Active,
                items: vec![
                    FrameItem::Change(Box::new(BattleChange::CrystalSelected(choice))),
                    FrameItem::Change(Box::new(BattleChange::BuffActInfoMarker(marker))),
                ],
            },
            SemanticFrame {
                owner: FrameOwner::Command,
                trigger: FrameTrigger::Active,
                items: vec![FrameItem::Change(Box::new(
                    BattleChange::CardStateSnapshot(hand),
                ))],
            },
        ];
        crate::engine::packet::timeline::project_for_version_with_absorb_map_layout(
            &frames,
            self.fight.version.unwrap_or_default(),
            self.absorb_hurt_map_layout,
        )
        .inspect_err(|error| tracing::warn!(?error, "crystal selection projection failed"))
        .ok()
    }
}
