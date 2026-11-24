use crate::engine::fight::rules::{ATTACKER_SIDE_UID, OwnedBattleSkill};

pub fn side_buff_skills(
    game: &config::GameDB,
    buffs: impl IntoIterator<Item = i32>,
) -> Result<Vec<OwnedBattleSkill>, String> {
    let mut owned = Vec::new();
    for buff_id in buffs {
        let buff = game
            .rouge2_buff
            .get(buff_id)
            .ok_or_else(|| format!("missing Rouge2 buff {buff_id}"))?;
        for placement in buff.passive_skill_id.split('|') {
            if !placement.starts_with("4#") {
                continue;
            }
            let id = placement
                .strip_prefix("4#0#0#")
                .ok_or_else(|| format!("unsupported Rouge2 side buff {buff_id}: {placement}"))?;
            let skill_id = id
                .parse::<i32>()
                .ok()
                .filter(|skill_id| game.skill.get(*skill_id).is_some())
                .ok_or_else(|| format!("invalid Rouge2 side skill {id} for buff {buff_id}"))?;
            owned.push(OwnedBattleSkill {
                owner_uid: ATTACKER_SIDE_UID,
                skill_id,
            });
        }
    }
    Ok(owned)
}
