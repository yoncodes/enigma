use std::path::Path;

use battle::tooling::replay::{OwnedBattleSkill, rouge2_side_buff_skills};
use sonettobuf::{
    Fight, Rouge2BagItemRemovePush, Rouge2BagItemUpdatePush, Rouge2InfoPush, Rouge2Item,
    custom_data::CustomDataType,
};

use super::metadata::{capture_command, capture_timeline_through, read_capture_value};

const ACTIVE_RUN: i32 = 2;
const BUFF_BAG: i32 = 3;
const BAG_UID_STRIDE: i32 = 100_000_000;

pub fn rouge2_side_skills(
    game: &config::GameDB,
    fight: &Fight,
    start: &Path,
) -> anyhow::Result<Vec<OwnedBattleSkill>> {
    if !fight
        .custom_data
        .iter()
        .any(|data| data.r#type == Some(CustomDataType::Rouge2 as i32))
    {
        return Ok(Vec::new());
    }
    let files = capture_timeline_through(start)?.ok_or_else(|| {
        anyhow::anyhow!("missing Rouge2 inventory timeline for {}", start.display())
    })?;
    let index = files
        .iter()
        .rposition(|file| is_snapshot(file))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "missing Rouge2 inventory snapshot before {}",
                start.display()
            )
        })?;
    let snapshot_path = &files[index];
    let snapshot: Rouge2InfoPush = serde_json::from_value(read_capture_value(snapshot_path)?)?;
    let info = snapshot
        .rouge2_info
        .ok_or_else(|| anyhow::anyhow!("missing Rouge2 info in {}", snapshot_path.display()))?;
    let state = info
        .state
        .ok_or_else(|| anyhow::anyhow!("missing Rouge2 state in {}", snapshot_path.display()))?;
    if state != ACTIVE_RUN {
        return Ok(Vec::new());
    }
    let bag = info.bag_info.ok_or_else(|| {
        anyhow::anyhow!(
            "missing active Rouge2 inventory in {}",
            snapshot_path.display()
        )
    })?;
    let mut buffs = bag
        .bags
        .into_iter()
        .filter(|bag| bag.bag_type == Some(BUFF_BAG))
        .flat_map(|bag| bag.items)
        .collect::<Vec<_>>();
    for file in &files[index + 1..] {
        match capture_command(file).as_deref() {
            Some("Rouge2BagItemUpdatePush") => {
                let update: Rouge2BagItemUpdatePush =
                    serde_json::from_value(read_capture_value(file)?)?;
                for item in update.items.into_iter().filter(is_buff_item) {
                    if let Some(existing) = buffs.iter_mut().find(|old| old.uid == item.uid) {
                        *existing = item;
                    } else {
                        buffs.push(item);
                    }
                }
            }
            Some("Rouge2BagItemRemovePush") => {
                let removed: Rouge2BagItemRemovePush =
                    serde_json::from_value(read_capture_value(file)?)?;
                buffs.retain(|item| !removed.items.iter().any(|old| old.uid == item.uid));
            }
            _ => {}
        }
    }
    rouge2_side_buff_skills(
        game,
        buffs
            .into_iter()
            .filter(|item| item.count.is_some_and(|count| count > 0))
            .filter_map(|item| item.item_id),
    )
    .map_err(anyhow::Error::msg)
}

fn is_snapshot(file: &Path) -> bool {
    matches!(
        capture_command(file).as_deref(),
        Some(
            "GetRouge2InfoReply"
                | "Rouge2InfoPush"
                | "EnterRouge2SelectDifficultyReply"
                | "EnterRouge2SelectCareerReply"
                | "Rouge2GainCareer1RewardReply"
                | "Rouge2SummonerActiveTalentReply"
                | "Rouge2SummonerResetTalentReply"
                | "Rouge2GainCareerAttrDropReply"
                | "Rouge2EndReply"
                | "Rouge2AbortReply"
        )
    )
}

fn is_buff_item(item: &Rouge2Item) -> bool {
    item.uid.is_some_and(|uid| uid / BAG_UID_STRIDE == BUFF_BAG)
}
