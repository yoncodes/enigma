use super::*;

fn healing_amount(
    current_hp: i32,
    start_subscriber: bool,
    event_triggered: bool,
    conditional: bool,
) -> i32 {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(current_hp),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    passive_skill: if start_subscriber {
                        vec![30470143, 200]
                    } else {
                        vec![30470143]
                    },
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1),
                    attr: Some(HeroAttribute {
                        hp: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let behavior = if conditional {
        ParsedBehavior::new(
            60179,
            "HealOrAddBuffByHp",
            vec![500, 0, AttrId::Attack.id(), 1_000, 1],
        )
    } else {
        ParsedBehavior::new(20001, "Heal", vec![0, AttrId::Attack.id(), 1_000])
    };
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            behavior,
            TargetRequest {
                code: 1,
                raw: Vec::new(),
            },
        )],
    });
    if start_subscriber {
        let mut subscriber = SkillEffectSlot::new(
            ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new()),
            TargetRequest::self_only(),
        );
        subscriber.conditions = vec![ParsedCondition {
            opcode: 34203,
            type_name: "UseSkillEffectTag".to_owned(),
            kind: ParsedConditionKind::ActiveSkillEffectTag(vec![4]),
            raw_args: vec!["4".to_owned()],
        }];
        subscriber.compiled_route =
            crate::engine::skill::rule::route::ConditionRoute::compile(&subscriber.conditions);
        catalog.insert(ParsedSkillEffect {
            skill_id: 200,
            slots: vec![subscriber],
        });
    }
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(11);

    let trigger = if event_triggered {
        SkillOpTrigger::Event(BattleEvent::Kind(EventKind::RoundEnd))
    } else {
        SkillOpTrigger::Active
    };
    emit_all_ops(
        invocation,
        &managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &trigger,
    )
    .unwrap()
    .into_iter()
    .find_map(|op| match op {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal))) => Some(heal.amount),
        _ => None,
    })
    .unwrap()
}

#[test]
fn conditional_passive_healing_done_reaches_a_runtime_heal() {
    assert_eq!(healing_amount(499, false, false, false), 1_100);
    assert_eq!(healing_amount(500, false, false, false), 1_000);
}

#[test]
fn healing_modifier_survives_an_action_start_continuation() {
    assert_eq!(healing_amount(499, true, false, false), 1_100);
}

#[test]
fn event_triggered_hp_branch_collects_passive_modifiers() {
    assert_eq!(healing_amount(499, false, true, true), 1_100);
}
