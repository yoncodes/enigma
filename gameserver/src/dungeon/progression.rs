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

#[derive(Clone, Copy)]
enum AdvancedConditionType {
    CasualtiesBelow = 1,
    RoundsAtMost = 2,
    NoCasualtiesWithinRounds = 3,
}

impl AdvancedConditionType {
    fn from_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(Self::CasualtiesBelow),
            2 => Some(Self::RoundsAtMost),
            3 => Some(Self::NoCasualtiesWithinRounds),
            _ => None,
        }
    }
}

pub fn battle_star(runtime: &battle::engine::runtime::BattleRuntime, battle_id: i32) -> i32 {
    let Some(battle) = configs::get().battle.get(battle_id) else {
        return 1;
    };
    let base_star = successful_battle_base_star(&battle.advanced_condition);
    let dead = runtime.dead_attacker_count() as i32;
    let round = runtime.current_round();

    let advanced_star = battle
        .advanced_condition
        .split('|')
        .filter_map(|id| id.parse::<i32>().ok())
        .filter_map(|id| configs::get().condition.get(id))
        .filter(|condition| {
            let limit = condition.attr.parse::<i32>().unwrap_or_default();
            match AdvancedConditionType::from_id(condition.r#type) {
                Some(AdvancedConditionType::CasualtiesBelow) => dead < limit,
                Some(AdvancedConditionType::RoundsAtMost) => round <= limit,
                Some(AdvancedConditionType::NoCasualtiesWithinRounds) => {
                    dead == 0 && round <= limit
                }
                None => {
                    tracing::warn!(
                        condition_id = condition.id,
                        condition_type = condition.r#type,
                        "unsupported dungeon advanced condition"
                    );
                    false
                }
            }
        })
        .count() as i32;
    base_star + advanced_star
}

fn successful_battle_base_star(advanced_condition: &str) -> i32 {
    1 + i32::from(advanced_condition.is_empty())
}
