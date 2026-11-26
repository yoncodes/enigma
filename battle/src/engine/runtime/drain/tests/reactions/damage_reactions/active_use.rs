use super::*;

#[test]
fn ally_attacked_heal_follows_target_higge_and_pays_once_for_two_hits() {
    crate::test_support::init_config();
    let entity = |uid, team, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(7_500),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, vec![31200222]), entity(11, 1, vec![31250144])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, vec![])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [40110421, 31200222, 31250144], []);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation = SkillInvocation::from(SkillRequest {
        source_uid: -1,
        skill_id: 40110421,
    });
    invocation.mode = SkillExecutionMode::Active;
    invocation.target = SkillTarget::Explicit(10);
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();

    assert!(result.events.iter().any(|event| matches!(
        event,
        BattleEvent::SkillAction(action)
            if action.phase == crate::engine::skill::action::SkillPhase::HitPassives
                && action.attacked_target_uids.len() == 2
    )));
    let ordered_events = result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::BuffAdded(change) if change.buff_id == 31200142 => Some("Higge"),
            BattleEvent::BuffAdded(change) if change.buff_id == 31250191 => Some("marker"),
            BattleEvent::HpHealed {
                source_uid: 11,
                target_uid: 10,
                amount: 1_200,
                ..
            } => Some("heal Nautika"),
            BattleEvent::HpHealed {
                source_uid: 11,
                target_uid: 11,
                amount: 1_200,
                ..
            } => Some("heal Rubuska"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_events,
        ["Higge", "marker", "heal Nautika", "heal Rubuska"]
    );
    assert_eq!(managers.buff.buff_id_amount(11, 31250191), 0);
}

#[test]
fn life_more_grants_pollution_after_the_owners_attack_above_the_threshold() {
    crate::test_support::init_config();
    let passive = 3120004;
    let attack = 312000411;
    let pollution = 3120001;
    let non_attack = 9000;
    let entity = |uid, team, hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut catalog = SkillEffectCatalog::from_roots(config::configs::get(), [passive, attack], []);
    catalog.insert(ParsedSkillEffect {
        skill_id: non_attack,
        slots: Vec::new(),
    });
    for (skill_id, hp, expected) in [
        (attack, 8_001, 2),
        (attack, 8_000, 1),
        (non_attack, 10_000, 0),
    ] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1, 10_000)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    passive_skill: vec![passive],
                    ..entity(-1, 2, hp)
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut invocation = SkillInvocation::from(SkillRequest {
            source_uid: -1,
            skill_id,
        });
        invocation.mode = SkillExecutionMode::Active;
        invocation.target = SkillTarget::Explicit(10);

        run(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_amount(10, pollution), expected);
        assert_eq!(managers.buff.buff_id_amount(-1, pollution), 0);
    }
}

#[test]
fn flinch_reaction_follows_cast_target_and_buffs_only_exposed_caster() {
    crate::test_support::init_config();
    let skill_id = 31460223;
    let passive_id = 725400130;
    let flinch = 90101;
    let entity = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(10000),
        attr: Some(HeroAttribute {
            hp: Some(10000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let ally = |uid| FightEntityInfo {
        model_id: Some(3146),
        skill_group2: vec![skill_id],
        buffs: vec![BuffInfo {
            uid: Some(uid + 100),
            buff_id: Some(72540007),
            from_uid: Some(-1),
            duration: Some(1),
            ..Default::default()
        }],
        ..entity(uid, 1)
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                ally(10),
                FightEntityInfo {
                    passive_skill: vec![passive_id],
                    ..ally(20)
                },
                entity(30, 1),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let cast = |source_uid, extra: bool| {
        let mut invocation = SkillInvocation::from(SkillRequest {
            source_uid,
            skill_id,
        });
        invocation.card_index = i32::from(!extra);
        invocation.mode = SkillExecutionMode::Active;
        invocation.target = SkillTarget::Explicit(30);
        invocation.extra_skill_kind =
            extra.then_some(crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction);

        let mut managers = BattleManagers::seeded(&fight);
        let result = run(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
        let parent = steps
            .iter()
            .find(|step| step.act_id == Some(skill_id))
            .expect("the test cast ran");
        assert_eq!(parent.to_id, Some(30));
        let reaction = parent
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find(|step| step.act_id == Some(passive_id))
            .cloned();
        assert_eq!(managers.buff.buff_id_amount(10, flinch), 0);
        assert_eq!(managers.buff.buff_id_amount(30, flinch), 0);
        let amount = managers.buff.buff_id_amount(20, flinch);
        assert_eq!(
            amount,
            if source_uid == 20 && !extra { 5 } else { 0 },
            "{steps:#?}"
        );
        (reaction, amount)
    };

    let (reaction, amount) = cast(20, false);
    let reaction = reaction.expect("Exposed wearer's active cast grants Flinch");
    assert_eq!(reaction.to_id, Some(30));
    assert_eq!(amount, 5);
    assert!(reaction.act_effect.iter().any(|effect| {
        effect.target_id == Some(20)
            && effect
                .buff
                .as_ref()
                .is_some_and(|buff| buff.buff_id == Some(flinch))
    }));

    let (ally_reaction, amount) = cast(10, false);
    assert_eq!(amount, 0);
    assert!(ally_reaction.is_none());
    let (extra_reaction, amount) = cast(20, true);
    assert_eq!(amount, 0);
    assert!(extra_reaction.is_none());
}
