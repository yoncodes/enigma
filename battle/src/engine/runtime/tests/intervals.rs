use super::*;

#[test]
fn defender_round_start_interval_counts_starts_with_or_without_extra_rounds() {
    crate::test_support::init_config();
    for extra in [false, true] {
        let entity = |uid, team, hp| FightEntityInfo {
            uid: Some(uid),
            team_type: Some(team),
            current_hp: Some(hp),
            attr: Some(sonettobuf::HeroAttribute {
                hp: Some(hp),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut runtime = runtime(Fight {
            version: Some(7),
            cur_round: Some(1),
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1, 10_000)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    passive_skill: vec![103360263],
                    ..entity(-1, 2, 200_000_000)
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut display = 1;
        for start in 1..=12_i32 {
            if extra && matches!(start, 4 | 10) {
                runtime
                    .round_state
                    .grant_extra_round(crate::engine::skill::action::SkillRequest {
                        source_uid: 10,
                        skill_id: 31460181,
                    });
            } else {
                display += 1;
            }
            let reply = runtime.advance_round(BeginRoundRequest::default()).unwrap();
            assert_eq!(reply.cur_round, Some(display));
            assert_eq!(
                runtime.managers.buff.buff_id_amount(-1, 109320116),
                (start - 10).max(0) * 2,
                "defender start {start}, extra rounds {extra}, display {display}"
            );
        }
    }
}
