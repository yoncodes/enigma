use super::*;

#[test]
fn sovereign_card_waives_ap_by_card_type() {
    let managers = BattleManagers::seeded(&Fight::default());
    let catalog = crate::engine::skill::effect::catalog::SkillEffectCatalog::default();
    let mut card = CardInfo {
        uid: Some(10),
        skill_id: Some(31472131),
        card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
        ..Default::default()
    };

    assert!(!CardMechanic.uses_action_point(&card, 10, 31472131, &managers, &catalog));
    card.card_type = None;
    assert!(CardMechanic.uses_action_point(&card, 10, 31472131, &managers, &catalog));
}

#[test]
fn common_ultimate_uses_base_gauge_before_cost_offset() {
    crate::test_support::init_config();
    for (moxie, ready) in [(4, false), (5, true)] {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3140),
                    current_hp: Some(100),
                    ex_point: Some(moxie),
                    ex_point_max: Some(6),
                    expoint_max_add: Some(-1),
                    ex_skill: Some(31402141),
                    buffs: [31400000, 31400010]
                        .into_iter()
                        .enumerate()
                        .map(|(index, buff_id)| BuffInfo {
                            uid: Some(index as i64 + 20),
                            buff_id: Some(buff_id),
                            from_uid: Some(10),
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers.ex_point.add_max(10, 2);
        let pool = TargetPool::from_fight(&fight);
        let unit = &pool.attacker_main[0];
        assert_eq!(managers.ex_point.maximum(10), Some(7));
        assert_eq!(CardMechanic.required_ultimate_cost(&managers, unit), 5);
        assert_eq!(
            !CardMechanic
                .special_team_cards(&pool, &managers, &[])
                .is_empty(),
            ready
        );
    }
}
