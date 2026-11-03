use super::*;
use crate::test_support::init_config;

#[test]
fn selected_build_uses_requested_psychube_and_destiny_levels() {
    init_config();
    let passives = Passive::for_config(3086, Some((1527, 4)), Some((308601, 3)));
    assert_eq!(
        passives,
        Passive::configured(
            crate::test_support::game_data(),
            3086,
            Some((1527, 4)),
            Some((308601, 3)),
        )
    );

    assert!(passives.iter().any(|passive| {
        passive.skill_id == 432714
            && passive.source.kind == PassiveSourceKind::Psychube
            && passive.source.rank == 4
    }));
}

#[test]
fn owned_insight_roots_are_filtered_before_portrait_upgrades() {
    let game = crate::test_support::game_data();
    for (rank, level, portrait, insight_three) in [
        (3, 71, 5, None),
        (4, 121, 0, Some(31490191)),
        (4, 121, 5, Some(31492191)),
    ] {
        let passives = Passive::for_build(
            game,
            &HeroBuildInput {
                hero_id: 3149,
                rank,
                level,
                ex_skill_level: portrait,
                ..Default::default()
            },
            &[],
            None,
        );
        assert_eq!(
            passives
                .iter()
                .find(|passive| passive.source.kind == PassiveSourceKind::Insight
                    && passive.source.rank == 3)
                .map(|passive| passive.skill_id),
            insight_three
        );
        for skill_id in [
            if portrait == 0 { 31490171 } else { 31495171 },
            31490181,
            31490161,
        ] {
            assert!(passives.iter().any(|passive| passive.skill_id == skill_id));
        }
    }
}
