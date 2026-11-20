use crate::engine::{
    manager::BattleManagers,
    skill::action::{SkillExecutionMode, SkillRateModifier},
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [rank_one, rank_two, rank_three, ..]
        if [rank_one, rank_two, rank_three].iter().all(|rate| **rate >= 0))
}

pub fn rate_modifiers(
    managers: &BattleManagers,
    source_uid: i64,
    skill_id: i32,
    is_big_skill: bool,
    main_target_uid: i64,
    mode: SkillExecutionMode,
) -> Vec<SkillRateModifier> {
    if main_target_uid == 0
        || !matches!(
            mode,
            SkillExecutionMode::Active | SkillExecutionMode::DirectBig
        )
    {
        return Vec::new();
    }
    let features = managers.buff.active_features(&managers.hp);
    let owned = |kind| {
        features
            .iter()
            .filter(move |feature| feature.owner_uid == source_uid && super::is_kind(feature, kind))
    };
    let rank = match managers.catalog().skill_rank(skill_id) {
        rank @ 1..=3 => rank,
        _ if is_big_skill && owned(BuffActKind::UseBigSkillLike3Rank).next().is_some() => 3,
        _ => return Vec::new(),
    };
    owned(BuffActKind::AttackSkillRateUp)
        .filter(|feature| {
            !feature
                .values
                .get(4..)
                .is_some_and(|excluded| excluded.contains(&skill_id))
        })
        .filter_map(|feature| {
            let rate = *feature.values.get(rank as usize)?;
            if rate <= 0 {
                return None;
            }
            Some(SkillRateModifier::fixed(
                main_target_uid,
                feature.act_id()?,
                rate,
                true,
            ))
        })
        .collect()
}

#[cfg(test)]
#[path = "attack_skill_rate_up/tests.rs"]
mod tests;
