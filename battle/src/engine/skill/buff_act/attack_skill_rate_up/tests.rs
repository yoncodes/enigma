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
