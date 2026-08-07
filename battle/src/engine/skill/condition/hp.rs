use crate::engine::{
    manager::BattleManagers,
    skill::{
        condition::parse::{ParsedConditionKind, first_i32, parse_fixed},
        target::TargetPool,
    },
};

pub fn per_hp(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerHp {
        interval_permille: first_i32(args)?,
    })
}

pub fn per_lost_hp(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [interval_permille] = parse_fixed(args)?;
    (interval_permille > 0).then_some(ParsedConditionKind::PerLostHp { interval_permille })
}

pub fn lost_hp_interval_count(uid: i64, interval_permille: i32, managers: &BattleManagers) -> i32 {
    let max = managers.hp.max(uid);
    if max <= 0 || interval_permille <= 0 {
        return 0;
    }
    let missing_permille =
        ((max - managers.hp.current(uid)).max(0) as i64 * 1000 / max as i64) as i32;
    missing_permille / interval_permille
}

pub fn team_lost_hp(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [team_type, interval_permille, max_count] = parse_fixed(args)?;
    Some(ParsedConditionKind::TeamLostHpPercent {
        team_type,
        interval_permille,
        max_count,
    })
}

pub fn team_lost_hp_count(
    team_type: i32,
    interval_permille: i32,
    max_count: i32,
    managers: &BattleManagers,
    pool: &TargetPool,
) -> i32 {
    if interval_permille <= 0 || max_count <= 0 {
        return 0;
    }
    let team = match team_type {
        1 => &pool.attacker_main,
        2 => &pool.defender_main,
        _ => return 0,
    };
    let (current, max) = team.iter().fold((0_i64, 0_i64), |(current, max), entity| {
        (
            current + i64::from(managers.hp.current(entity.uid).max(0)),
            max + i64::from(managers.hp.max(entity.uid).max(0)),
        )
    });
    if max <= 0 {
        return 0;
    }
    let missing_permille = (max - current).max(0).saturating_mul(1000) / max;
    (missing_permille / i64::from(interval_permille))
        .min(i64::from(max_count))
        .clamp(0, i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_per_hp_condition() {
        assert_eq!(
            per_hp(744203, "PerHp", &["200".into()]),
            Some(ParsedConditionKind::PerHp {
                interval_permille: 200,
            })
        );
    }

    #[test]
    fn parses_exact_per_lost_hp_condition() {
        assert_eq!(
            per_lost_hp(12203, "LostLifePer", &["100".into()]),
            Some(ParsedConditionKind::PerLostHp {
                interval_permille: 100,
            })
        );
    }

    #[test]
    fn parses_team_lost_hp_steps_without_collapsing_the_exact_key() {
        let args = ["1".into(), "50".into(), "5".into()];
        assert_eq!(
            team_lost_hp(697101, "TeamLostHpPercent", &args),
            Some(ParsedConditionKind::TeamLostHpPercent {
                team_type: 1,
                interval_permille: 50,
                max_count: 5,
            })
        );
    }
}
