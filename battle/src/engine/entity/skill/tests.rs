use super::{Skill, parse_skill_group};
use crate::test_support::init_config;

#[test]
fn parses_requested_skill_group() {
    assert_eq!(parse_skill_group("1#10#11|2#20#21", 2), vec![20, 21]);
}

#[test]
fn parses_only_the_first_choice_family() {
    assert_eq!(
        parse_skill_group("1#10#11#12,20#21#22|2#30#31", 1),
        vec![10, 11, 12]
    );
}

#[test]
fn nautika_portrayal_tiers_keep_only_the_primary_choice_family() {
    init_config();
    let game = crate::test_support::game_data();
    for (level, expected) in [
        (1, vec![312001213, 312001223, 312001233]),
        (3, vec![312001214, 312001224, 312001234]),
        (4, vec![312001215, 312001225, 312001235]),
    ] {
        let (_, group2, _) = Skill::active_skills(game, 3120, 3, level);
        assert_eq!(group2, expected, "portrayal level {level}");
    }
}

#[test]
fn nautika_swaps_kit_and_gains_ultimate_at_insight_two() {
    init_config();
    let game = crate::test_support::game_data();
    assert_eq!(
        Skill::active_skills(game, 3120, 2, 5),
        (
            vec![31200201, 31200202, 31200203],
            vec![31200211, 31200212, 31200213],
            0,
        )
    );
    assert_eq!(
        Skill::active_skills(game, 3120, 3, 0),
        (
            vec![31200111, 31200112, 31200113],
            vec![31200121, 31200122, 31200123],
            31200131,
        )
    );
    assert_eq!(Skill::active_skills(game, 3120, 3, 1).2, 31200131);
}

#[test]
fn paper_heron_initial_loadout_keeps_only_the_primary_choice_family() {
    init_config();
    let game = crate::test_support::game_data();
    assert_eq!(
        Skill::active_skills(game, 3135, 0, 0),
        (
            vec![31350111, 31350112, 31350113],
            vec![31350121, 31350122, 31350123],
            31350131,
        )
    );
    assert_eq!(
        Skill::active_skills(game, 3135, 0, 1).0,
        vec![31350114, 31350115, 31350116]
    );
}

#[test]
fn selected_level_replaces_active_skills_and_ultimate_cumulatively() {
    init_config();

    assert_eq!(
        Skill::active_skills(crate::test_support::game_data(), 3134, 0, 5),
        (
            vec![31345111, 31345112, 31345113],
            vec![31344121, 31344122, 31344123],
            31345131,
        )
    );
}
