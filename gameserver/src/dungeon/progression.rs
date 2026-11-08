use super::*;

pub use logic::dungeon::episode_cost_value;

pub struct InstructionDungeonRewardClaim {
    pub reply: InstructionDungeonRewardReply,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub struct InstructionDungeonFinalRewardClaim {
    pub reply: InstructionDungeonFinalRewardReply,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
}

pub async fn can_start_episode(
    db: &SqlitePool,
    player_id: i64,
    chapter_id: i32,
    episode_id: i32,
) -> Result<bool, AppError> {
    Ok(dungeons::can_start_episode(db, player_id, chapter_id, episode_id).await?)
}

pub async fn episode_battle_id(
    db: &SqlitePool,
    player_id: i64,
    episode: &config::episode::Episode,
) -> Result<i32, AppError> {
    let star = dungeons::episode_star(db, player_id, episode.id).await?;
    Ok(if star <= 0 && episode.first_battle_id > 0 {
        episode.first_battle_id
    } else {
        episode.battle_id
    })
}

pub fn episode_cost(episode: &config::episode::Episode, multiplier: i32) -> reward::RewardSet {
    let mut cost = reward::parse(&episode.cost);
    cost.scale(multiplier.max(1));
    cost
}

pub fn failure_refund(episode: &config::episode::Episode, multiplier: i32) -> reward::RewardSet {
    let mut refund = episode_cost(episode, multiplier);
    let retained = reward::parse(&episode.fail_cost);
    subtract_costs(&mut refund.items, &retained.items);
    subtract_costs(&mut refund.currencies, &retained.currencies);
    refund
}

fn subtract_costs<T: Eq>(costs: &mut Vec<(T, i32)>, retained: &[(T, i32)]) {
    for (id, amount) in retained {
        if let Some((_, refundable)) = costs.iter_mut().find(|(cost_id, _)| cost_id == id) {
            *refundable = (*refundable - amount).max(0);
        }
    }
    costs.retain(|(_, amount)| *amount > 0);
}

pub fn battle_star(runtime: &battle::Battle, battle_id: i32) -> Result<i32, AppError> {
    let tables = configs::get();
    if battle_id == 0 {
        return Ok(1);
    }
    let Some(battle) = tables.battle.get(battle_id) else {
        return Err(AppError::Custom(format!(
            "missing battle config {battle_id}"
        )));
    };
    let base_star = successful_battle_base_star(&battle.advanced_condition);
    let advanced_star =
        advanced_condition_star(runtime, battle_id, &battle.advanced_condition, tables)?;
    Ok(base_star + advanced_star)
}

fn advanced_condition_star(
    runtime: &battle::Battle,
    battle_id: i32,
    raw: &str,
    tables: &config::GameDB,
) -> Result<i32, AppError> {
    if raw.is_empty() {
        return Ok(0);
    }
    let mut star = 0;
    for raw_id in raw.split('|') {
        let condition_id = raw_id.parse::<i32>().map_err(|_| {
            AppError::Custom(format!(
                "battle {battle_id} has malformed advanced condition id {raw_id:?}"
            ))
        })?;
        let condition = tables.condition.get(condition_id).ok_or_else(|| {
            AppError::Custom(format!(
                "battle {battle_id} references missing advanced condition {condition_id}"
            ))
        })?;
        let limit = condition_limit(battle_id, condition)?;
        match runtime.meets_advanced_condition(condition.r#type, limit) {
            Some(true) => star += 1,
            Some(false) => {}
            None => tracing::warn!(
                battle_id,
                condition_id,
                condition_type = condition.r#type,
                "unsupported dungeon advanced condition"
            ),
        }
    }
    Ok(star)
}

fn condition_limit(
    battle_id: i32,
    condition: &config::condition::Condition,
) -> Result<i32, AppError> {
    condition.attr.parse::<i32>().map_err(|_| {
        AppError::Custom(format!(
            "battle {battle_id} condition {} has malformed attr {:?}",
            condition.id, condition.attr
        ))
    })
}

fn successful_battle_base_star(advanced_condition: &str) -> i32 {
    1 + i32::from(advanced_condition.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> battle::Battle {
        battle::tooling::unstarted_battle(
            battle::catalog::BattleCatalog::new(configs::get()),
            sonettobuf::Fight::default(),
        )
        .unwrap()
    }

    #[test]
    fn malformed_advanced_conditions_are_errors() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = config::init(data_dir.to_str().unwrap());
        let runtime = runtime();
        let tables = configs::get();

        for raw in ["bad", "2147483647", "1||2"] {
            assert!(advanced_condition_star(&runtime, 1, raw, tables).is_err());
        }
        let condition = config::condition::Condition {
            attr: "bad".to_owned(),
            desc: String::new(),
            id: 7,
            progress: 0,
            r#type: 7,
        };
        assert!(condition_limit(1, &condition).is_err());
    }

    #[test]
    fn only_the_no_battle_sentinel_uses_the_base_star() {
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data/excel2json");
        let _ = config::init(data_dir.to_str().unwrap());
        let runtime = runtime();

        assert_eq!(battle_star(&runtime, 0).unwrap(), 1);
        assert!(battle_star(&runtime, i32::MAX).is_err());
    }
}
