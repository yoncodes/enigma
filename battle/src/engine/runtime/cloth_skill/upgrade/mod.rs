use sonettobuf::{FightStep, UseClothSkillRequest};

use super::{BattleRuntime, drain, project_changes, project_result};
use crate::engine::runtime::{
    change::BattleChange,
    record::{FrameItem, FrameOwner, FrameTrigger, SemanticFrame},
};

pub(super) fn apply(
    runtime: &mut BattleRuntime,
    request: &UseClothSkillRequest,
) -> Option<Vec<FightStep>> {
    let owner_uid = request.from_id?;
    let upgrade_id = request.skill_id?;
    let option_id = i32::try_from(request.to_id?).ok()?;
    let applied =
        runtime
            .managers
            .select_upgrade(&mut runtime.fight, owner_uid, upgrade_id, option_id)?;
    let version = runtime.fight.version.unwrap_or_default();
    let layout = runtime.absorb_hurt_map_layout;
    let result = if version == 7 {
        let team_type = applied.entity.team_type?;
        let buff_change = |change| BattleChange::Buff(Box::new(change));
        let identity = BattleChange::Card(Box::new(applied.card_changes));
        let items = applied
            .removed_buffs
            .into_iter()
            .map(buff_change)
            .chain(std::iter::once(identity))
            .chain(applied.granted_buffs.into_iter().map(buff_change))
            .map(|change| FrameItem::Change(Box::new(change)))
            .collect();
        let snapshot = runtime.managers.card.state_snapshot(team_type);
        project_result(
            drain::DrainResult {
                frames: vec![
                    SemanticFrame {
                        owner: FrameOwner::EventEffect {
                            source_uid: owner_uid,
                            target_uid: 0,
                        },
                        trigger: FrameTrigger::Active,
                        items,
                    },
                    SemanticFrame {
                        owner: FrameOwner::Command,
                        trigger: FrameTrigger::Active,
                        items: vec![FrameItem::Change(Box::new(
                            BattleChange::CardStateSnapshot(snapshot),
                        ))],
                    },
                ],
                ..Default::default()
            },
            version,
            layout,
        )
    } else {
        project_changes(
            [BattleChange::UpgradeApplied(Box::new(applied))],
            version,
            layout,
        )
    };
    result
        .inspect_err(|error| tracing::warn!(%error, "hero upgrade projection failed"))
        .ok()
}
