use sonettobuf::CardInfo;

use crate::engine::skill::{rule::CommandOrigin, target::TargetPool};

use super::pool::card_for_target;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedAiSkill {
    pub owner_uid: i64,
    pub skill_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardPrepareAiSkill {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub skill_id: i32,
}

pub(crate) fn cards(
    pool: &TargetPool,
    prepared: &[PreparedAiSkill],
) -> Result<Vec<CardInfo>, String> {
    prepared
        .iter()
        .map(|prepared| {
            let owner = pool
                .defender_main
                .iter()
                .find(|entity| {
                    entity.uid == prepared.owner_uid && entity.uid != 0 && entity.current_hp > 0
                })
                .ok_or_else(|| {
                    format!("prepared AI owner {} is not eligible", prepared.owner_uid)
                })?;
            card_for_target(owner, prepared.skill_id)
                .ok_or_else(|| format!("invalid prepared AI skill {}", prepared.skill_id))
        })
        .collect()
}
