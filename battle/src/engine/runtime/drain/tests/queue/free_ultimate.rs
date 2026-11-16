use super::*;

#[test]
fn free_ultimate_runs_the_current_skill_below_its_normal_cost() {
    crate::test_support::init_config();
    let unit = |uid, team, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        position: Some(position),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    model_id: Some(3146),
                    skill_group1: vec![31460111, 31460112, 31460113],
                    skill_group2: vec![31460121, 31460122, 31460123],
                    ex_skill: Some(31460131),
                    ex_point: Some(2),
                    ex_point_max: Some(5),
                    ..unit(10, 1, 1)
                },
                unit(11, 1, 2),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![unit(-1, 2, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let hand = vec![CardInfo {
        uid: Some(11),
        skill_id: Some(30941111),
        ..Default::default()
    }];
    managers.card.resync_hand(hand.clone());
    let mut catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31460131], []);
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(60196, "DirectUseExSkillNoConsumeExPoint", vec![]),
            TargetRequest::self_only(),
        )],
    });

    let source = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    };
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(source.into())],
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 2);
    assert_eq!(managers.card.hand(), hand);
    assert!(managers.buff.has_buff_id(11, 31460131));
    assert!(managers.buff.has_buff_id(10, 31460133));
    assert_eq!(
        result
            .events
            .iter()
            .filter(|event| matches!(
                event, BattleEvent::AllyAction(action) if action.skill_id == 31460131
                    && action.skill_slot == 3 && action.moxie_spent == 0 && action.card_index == 0
            ))
            .count(),
        1
    );
    let steps = crate::engine::packet::timeline::project_for_version(&result.frames, 7).unwrap();
    let parent = steps.iter().find(|step| step.act_id == Some(100)).unwrap();
    let nested = parent
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .filter(|step| step.act_id == Some(31460131))
        .collect::<Vec<_>>();
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].act_type, Some(1));
}
