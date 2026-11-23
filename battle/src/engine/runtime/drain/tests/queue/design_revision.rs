use super::*;

#[test]
fn design_revision_threshold_belongs_to_the_committed_grant() {
    crate::test_support::init_config();
    let as_found = 31402162;
    let grant = 42;
    let observer = 43;
    let iteration = 31402004;
    let revision = 31400005;
    let observed = 90101;
    let entity = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(1000),
        attr: Some(HeroAttribute {
            hp: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                model_id: Some(3140),
                passive_skill: vec![as_found, observer],
                ..entity(10, 1)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let add = |id, amount| {
        SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", vec![id, amount]),
            TargetRequest::self_only(),
        )
    };
    catalog.insert(ParsedSkillEffect {
        skill_id: grant,
        slots: vec![add(iteration, 8)],
    });
    let mut observer_slot = add(observed, 1);
    observer_slot.conditions = crate::engine::skill::condition::parse_conditions(
        config::configs::get(),
        &format!("581#{iteration}#8"),
    );
    observer_slot.compiled_route = ConditionRoute::compile(&observer_slot.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: observer,
        slots: vec![observer_slot],
    });

    let mut managers = BattleManagers::seeded(&fight);
    let result = run_skill(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SkillRequest {
            source_uid: 10,
            skill_id: grant,
        }
        .into(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(managers.buff.buff_id_amount(10, iteration), 8);
    assert_eq!(managers.buff.buff_id_amount(10, revision), 1);
    assert_eq!(managers.buff.buff_id_amount(10, observed), 1);

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let [step] = steps.as_slice() else {
        panic!("expected one causing action")
    };
    assert_eq!(step.act_id, Some(grant));
    let grants = step
        .act_effect
        .iter()
        .filter_map(|effect| effect.buff.as_ref()?.buff_id);
    assert_eq!(grants.collect::<Vec<_>>(), vec![iteration, revision]);
    let nested = step
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref()?.act_id);
    assert_eq!(nested.collect::<Vec<_>>(), vec![observer]);
}
