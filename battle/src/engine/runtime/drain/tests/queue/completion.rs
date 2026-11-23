use super::*;

#[test]
fn selected_cast_completes_its_slots_after_the_hit() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1000),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30940191),
                        duration: Some(1),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1000),
                    passive_skill: vec![100],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let slot = |condition, behavior| {
        let mut slot = SkillEffectSlot::new(behavior, TargetRequest::self_only());
        slot.condition_target = TargetRequest {
            code: 101,
            raw: Vec::new(),
        };
        slot.conditions =
            crate::engine::skill::condition::parse_conditions(config::configs::get(), condition);
        slot.compiled_route = ConditionRoute::compile(&slot.conditions);
        slot
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![
            slot(
                "212",
                ParsedBehavior::new(60010, "DisperseForce2", vec![30940191]),
            ),
            slot("210|212", ParsedBehavior::new(1, "AddBuff", vec![4150001])),
        ],
    });
    catalog.insert_logic_target(100, 103);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation.clone(),
    )
    .unwrap();

    let changes = result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change) => {
                Some(("add", change.buff_id, change.target_uid))
            }
            BattleEvent::BuffRemoved(change) => Some(("remove", change.buff_id, change.target_uid)),
            BattleEvent::BuffRejected(change) => {
                Some(("reject", change.buff_id, change.target_uid))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    // The 210 grant runs before the 212 removal, so the held marker refuses Burn on its holder.
    assert_eq!(
        changes,
        [
            ("reject", 4150001, 10),
            ("remove", 30940191, 10),
            ("add", 4150001, 11),
        ]
    );

    let team = fight.attacker.as_mut().unwrap();
    team.entitys.truncate(1);
    team.entitys[0].passive_skill = vec![100];
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();
    assert_eq!(
        result
            .outcomes
            .iter()
            .filter(|outcome| {
                matches!(outcome, RuleOutcome::Buff(changes) if changes.origin.key.opcode == 60010)
            })
            .count(),
        1
    );
    let rendered = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let cast = rendered
        .iter()
        .find(|step| step.act_id == Some(100))
        .unwrap();
    assert!(
        cast.act_effect
            .iter()
            .any(|effect| effect.config_effect == Some(60010))
    );
}
