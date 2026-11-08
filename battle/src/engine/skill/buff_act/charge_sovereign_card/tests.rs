use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        card::{CARD_PLAY_ORIGIN, CardCommand, CardInsertSovereign, CardPlay, CardSetup},
    },
    packet::timeline,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::{SkillActionEvent, SkillExecutionMode, SkillInvocation, SkillPhase, SkillRequest},
        buff_act::{BuffActRuleError, registry},
        effect::SkillEffectCatalog,
        rule::output::RuleOp,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{
    BuffInfo, CardInfo, Fight, FightEntityInfo, FightStep, FightTeam, HeroAttribute,
    effect_type_enum::EffectType,
};

fn find_step(steps: &[FightStep], act_id: i32) -> Option<&FightStep> {
    for step in steps {
        if step.act_id == Some(act_id) {
            return Some(step);
        }
        for effect in &step.act_effect {
            if let Some(child) = &effect.fight_step
                && let Some(found) = find_step(std::slice::from_ref(child), act_id)
            {
                return Some(found);
            }
        }
    }
    None
}

fn fight() -> Fight {
    let actor = |uid, model_id, team_type| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(team_type),
        current_hp: Some(10000),
        attr: Some(HeroAttribute {
            hp: Some(10000),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut owner = actor(10, 3147, 1);
    owner.buffs = vec![BuffInfo {
        uid: Some(20),
        buff_id: Some(31470008),
        from_uid: Some(10),
        ..Default::default()
    }];
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![owner, actor(11, 3146, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: [-1, -2, -3]
                .into_iter()
                .map(|uid| actor(uid, 1, 2))
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn setup(fight: &Fight) -> BattleManagers {
    let mut managers = BattleManagers::seeded(fight);
    managers
        .execute_card(CardCommand::InsertSovereign(CardInsertSovereign {
            origin: CARD_PLAY_ORIGIN,
            target_uid: 10,
            skill_id: 31472131,
            hero_id: 3147,
            max_position: 9,
            team_type: 1,
        }))
        .unwrap();
    managers
}

#[test]
fn ap_mass_cast_charges_once() {
    crate::test_support::init_config();
    let fight = fight();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = setup(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31473121], [31470008]);
    let mut invocation = SkillInvocation::from(SkillRequest {
        source_uid: 10,
        skill_id: 31473121,
    });
    invocation.card_index = 1;
    invocation.mode = SkillExecutionMode::Active;
    let result = drain::run_command_group(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(invocation)],
    )
    .unwrap();
    assert_eq!(
        managers.card.hand()[0].card_dataes[0].value.as_deref(),
        Some(r#"{"lock":true,"strengthen":{"1":0,"2":1,"3":0,"4":0}}"#)
    );
    assert!(
        [-1, -2, -3]
            .into_iter()
            .all(|uid| managers.hp.current(uid) < 10000)
    );
    let projected = timeline::project_for_version(&result.frames, 7).unwrap();
    let charge = find_step(&projected, 31470008).unwrap();
    assert_eq!((charge.from_id, charge.to_id), (Some(10), Some(10)));
    assert!(!charge.fake_timeline.unwrap_or_default());
    assert_eq!(charge.act_effect.len(), 1);
    assert_eq!(
        charge.act_effect[0].effect_type,
        Some(EffectType::Updatecarddata as i32)
    );
    assert_eq!(charge.act_effect[0].effect_num, Some(1));
}

fn action(
    catalog: &SkillEffectCatalog,
    source_uid: i64,
    skill_id: i32,
    card_index: i32,
    mode: SkillExecutionMode,
) -> SkillActionEvent {
    SkillActionEvent {
        source_uid,
        card_index,
        skill_id,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: Vec::new(),
        phase: SkillPhase::AfterHit,
        skill_slot: 1,
        is_attack: catalog.is_attack(skill_id),
        rank: 1,
        skill_type: 0,
        effect_tag: catalog.effect_tag(skill_id),
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 0,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    }
}

#[test]
fn charging_respects_the_card_area_and_cap() {
    crate::test_support::init_config();
    let db = config::configs::get();
    let parsed = registry::resolve_feature(Some(db), "1135#1,3#2,4#5,6#7,8").unwrap();
    assert!(parsed.arguments_supported);
    assert_eq!(parsed.args, [1, 3, 2, 4, 5, 6, 7, 8]);
    assert!(
        !registry::resolve_feature(Some(db), "1135#1#3#2#4#5#6#7#8")
            .unwrap()
            .arguments_supported
    );
    let fight = fight();
    let pool = TargetPool::from_fight(&fight);
    let mut managers = setup(&fight);
    let catalog =
        SkillEffectCatalog::from_roots(db, [31470111, 31460133, 30840121, 31450121], [31470008]);
    let publish = |managers: &mut BattleManagers, event| {
        drain::run_event(
            managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            BattleEvent::SkillAction(event),
        )
    };
    let before = managers.card.clone();
    let mut early = action(&catalog, 11, 31460133, 1, SkillExecutionMode::Active);
    early.phase = SkillPhase::AfterDamage;
    for event in [
        early,
        action(&catalog, 10, 31470111, 0, SkillExecutionMode::Active),
        action(&catalog, 11, 31450121, 1, SkillExecutionMode::Device),
        action(&catalog, -1, 31470111, 1, SkillExecutionMode::Active),
    ] {
        publish(&mut managers, event).unwrap();
        assert_eq!(managers.card, before);
    }
    publish(
        &mut managers,
        action(&catalog, 11, 30840121, 2, SkillExecutionMode::Active),
    )
    .unwrap();
    let mut last = None;
    for index in 3..=7 {
        last = Some(
            publish(
                &mut managers,
                action(&catalog, 11, 31460133, index, SkillExecutionMode::Active),
            )
            .unwrap(),
        );
    }
    assert_eq!(
        managers.card.hand()[0].card_dataes[0].value.as_deref(),
        Some(r#"{"lock":true,"strengthen":{"1":0,"2":0,"3":4,"4":1}}"#)
    );
    assert!(
        find_step(
            &timeline::project_for_version(&last.unwrap().frames, 7).unwrap(),
            31470008
        )
        .is_none()
    );
    managers
        .execute_card(CardCommand::UnlockSovereign {
            origin: CARD_PLAY_ORIGIN,
            owner_uid: 10,
            team_type: 1,
        })
        .unwrap();
    let sovereign = managers.card.hand()[0].clone();
    let normal = CardInfo {
        uid: Some(10),
        skill_id: Some(31470111),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![normal.clone(), normal, sovereign],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    for _ in 0..3 {
        managers
            .execute_card(CardCommand::Play(CardPlay {
                origin: CARD_PLAY_ORIGIN,
                hand_index: 0,
                target_uid: None,
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
    }
    let before = managers.card.clone();
    let Err(drain::DrainError::BuffActRule(BuffActRuleError::ChargeSovereignCard(error))) = publish(
        &mut managers,
        action(&catalog, 10, 31470111, 1, SkillExecutionMode::Active),
    ) else {
        panic!("queued charging must remain explicit");
    };
    assert_eq!(
        error.failure,
        super::ChargeFailure::QueuedCard { card_index: 3 }
    );
    assert_eq!(error.args, Some([1, 3, 2, 4, 5, 6, 7, 8]));
    assert_eq!(managers.card, before);
}
