use super::*;

#[test]
fn ultimate_attributes_reach_preselection_and_settlement() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    for (skill_id, damage) in [(30620111, 1_000), (30620137, 2_008)] {
        let mut source = entity(10);
        source.buffs.push(BuffInfo {
            uid: Some(20),
            buff_id: Some(30620121),
            layer: Some(2),
            from_uid: Some(10),
            ..Default::default()
        });
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![source],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            10,
            &HeroExAttribute {
                cri: Some(900),
                cri_dmg: Some(1_500),
                ..Default::default()
            },
        );
        let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        catalog.insert_alias(skill_id, skill_id);
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: Vec::new(),
        });
        catalog.insert_damage_rate(skill_id, 1_000);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id,
        }
        .into();
        invocation.phase = Some(crate::engine::skill::action::SkillPhase::Damage);
        invocation.target = SkillTarget::Explicit(-1);
        let seed = (0..1000)
            .find(|seed| !RoundDeterminism::with_seed(*seed).roll_crit(skill_id, 10, -1, 900))
            .unwrap();
        run(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::with_seed(seed),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        assert_eq!(managers.hp.current(-1), 100_000 - damage);
        assert_eq!(managers.buff.buff_id_amount(10, 30620121), 2);
        assert_eq!(managers.origin_attribute(10, AttrId::CriticalRate), 900);
        assert_eq!(managers.origin_attribute(10, AttrId::CriticalDmg), 1_500);
    }
}

#[test]
fn primary_ultimate_uses_preparatory_credit_and_excludes_its_own_payment_credit() {
    crate::test_support::init_config();
    let ultimate = 31570114;
    let spend_passive = 434425;
    let unrelated_passive = 999001;
    let unrelated_buff = 434924;
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                ex_point: Some(5),
                ex_skill: Some(ultimate),
                passive_skill: vec![spend_passive, unrelated_passive],
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
    let db = config::configs::get();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(1_000),
            cri_dmg: Some(1_500),
            ..Default::default()
        },
    );
    let mut catalog = SkillEffectCatalog::from_roots(db, [ultimate, spend_passive], []);
    catalog.insert(ParsedSkillEffect {
        skill_id: ultimate,
        slots: Vec::new(),
    });
    catalog.insert_damage_rate(ultimate, 1_000);
    let mut observer = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![unrelated_buff]),
        TargetRequest::self_only(),
    );
    observer.conditions = crate::engine::skill::condition::parse::parse_conditions(db, "52008");
    observer.compiled_route = ConditionRoute::compile(&observer.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: unrelated_passive,
        slots: vec![observer],
    });
    let request = SkillRequest {
        source_uid: 10,
        skill_id: ultimate,
    };
    let invocation = SkillInvocation {
        mode: SkillExecutionMode::Active,
        target: SkillTarget::Explicit(-1),
        ..request.into()
    };
    let spend = |delta| {
        ExPointCommand::Spend(ExPointChange {
            origin: CARD_PLAY_ORIGIN,
            source_uid: 10,
            target_uid: 10,
            delta,
            config_effect: 0,
            effect_type: 0,
        })
    };
    let mut determinism = RoundDeterminism::default();
    let result = run_action_with_cost(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [RuleOp::DebitUltimate {
            action: request,
            cost: spend(-2),
        }],
        Some(spend(-3)),
        invocation.clone(),
    )
    .unwrap();
    let before_hit = result
        .events
        .iter()
        .take_while(|event| !matches!(event, BattleEvent::Hit(hit) if hit.skill_id == ultimate));
    let mut credits = Vec::new();
    let mut unrelated = Vec::new();
    for event in before_hit {
        if let BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change) = event {
            if change.buff_id == spend_passive {
                credits.push(change.after_amount);
            } else if change.buff_id == unrelated_buff {
                unrelated.push(change.after_amount);
            }
        }
    }
    assert_eq!(credits, [2, 5]);
    assert_eq!(unrelated, [1, 2]);
    assert_eq!(managers.ex_point.get(10), 0);
    assert_eq!(managers.hp.current(-1), 100_000 - 1_554);

    run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [],
        invocation,
    )
    .unwrap();
    assert_eq!(managers.hp.current(-1), 100_000 - 1_554 - 1_614);
}
