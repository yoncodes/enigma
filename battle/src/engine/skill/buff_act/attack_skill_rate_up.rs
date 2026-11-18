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
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;

    fn managers(buff_ids: &[i32]) -> BattleManagers {
        crate::test_support::init_config();
        BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: buff_ids
                        .iter()
                        .enumerate()
                        .map(|(index, buff_id)| BuffInfo {
                            uid: Some(20 + index as i64),
                            buff_id: Some(*buff_id),
                            from_uid: Some(10),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn rates(
        managers: &BattleManagers,
        skill_id: i32,
        big: bool,
        mode: SkillExecutionMode,
    ) -> Vec<(i64, Option<i32>)> {
        rate_modifiers(managers, 10, skill_id, big, -1, mode)
            .into_iter()
            .map(|rate| (rate.target_uid, rate.fixed_value()))
            .collect()
    }

    #[test]
    fn song_of_passion_adds_the_incantation_rank_rate_to_the_main_target() {
        let managers = managers(&[30870321]);

        assert_eq!(
            rates(&managers, 30060117, false, SkillExecutionMode::Active),
            vec![(-1, Some(500))]
        );
        assert_eq!(
            rates(&managers, 30060118, false, SkillExecutionMode::Active),
            vec![(-1, Some(1200))]
        );
        assert!(rates(&managers, 30060118, false, SkillExecutionMode::Nested).is_empty());
        assert!(rates(&managers, 30060131, true, SkillExecutionMode::Active).is_empty());
    }

    #[test]
    fn hidden_ritual_rank_counts_an_ultimate_as_rank_three() {
        let managers = managers(&[30870321, 30870303]);

        assert_eq!(
            rates(&managers, 30060131, true, SkillExecutionMode::Active),
            vec![(-1, Some(2000))]
        );
        assert!(rates(&managers, 31230132, true, SkillExecutionMode::Active).is_empty());
    }
}
