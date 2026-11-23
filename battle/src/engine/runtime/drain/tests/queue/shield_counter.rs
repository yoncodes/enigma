use super::*;

const CARRIER: i32 = 30940181;
const TEMPERING: i32 = 30940171;
const FIREPOWER: i32 = 530000131;
const TRENCH_DEFENSE: i32 = 530000111;
const J_SHIELD: i32 = 30940121;
const KIPERINA_SHIELD: i32 = 31170002;

fn setup(completion_passive: Vec<i32>) -> (BattleManagers, TargetPool, SkillEffectCatalog) {
    setup_with_shield(completion_passive, J_SHIELD, 10)
}

fn setup_with_shield(
    completion_passive: Vec<i32>,
    shield_id: i32,
    shield_from: i64,
) -> (BattleManagers, TargetPool, SkillEffectCatalog) {
    crate::test_support::init_config();
    let entity = |uid: i64, position| FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        current_hp: Some(100_000),
        buffs: if uid > 0 {
            vec![BuffInfo {
                uid: Some(30 + uid),
                buff_id: Some(shield_id),
                duration: Some(2),
                from_uid: Some(shield_from),
                ..Default::default()
            }]
        } else {
            Vec::new()
        },
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                {
                    let mut j = entity(10, 1);
                    j.buffs.push(BuffInfo {
                        uid: Some(20),
                        buff_id: Some(CARRIER),
                        from_uid: Some(10),
                        ..Default::default()
                    });
                    j
                },
                entity(11, 2),
                entity(12, 3),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-3, 1),
                FightEntityInfo {
                    passive_skill: completion_passive,
                    skill_group1: vec![TRENCH_DEFENSE],
                    ..entity(-5, 2)
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    for uid in [10, 11, 12] {
        managers.hp.set_shield(uid, 10_000);
    }
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [FIREPOWER, TRENCH_DEFENSE, TEMPERING],
        [CARRIER],
    );
    (managers, pool, catalog)
}

fn cast(source_uid: i64, skill_id: i32, mode: SkillExecutionMode) -> RuleOp {
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid,
        skill_id,
    }
    .into();
    invocation.mode = mode;
    invocation.target = SkillTarget::Explicit(11);
    RuleOp::Skill(invocation)
}

fn execute(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    ops: impl IntoIterator<Item = RuleOp>,
) -> DrainResult {
    run(
        managers,
        pool,
        catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        ops,
    )
    .unwrap()
}

fn steps_in(steps: &[sonettobuf::FightStep]) -> Vec<&sonettobuf::FightStep> {
    fn walk<'a>(step: &'a sonettobuf::FightStep, out: &mut Vec<&'a sonettobuf::FightStep>) {
        out.push(step);
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    for step in steps {
        walk(step, &mut out);
    }
    out
}

fn children(step: &sonettobuf::FightStep) -> impl Iterator<Item = &sonettobuf::FightStep> {
    step.act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
}

fn assert_group(step: &sonettobuf::FightStep, target_uid: i64, count: usize) {
    assert_eq!(
        (step.act_type, step.from_id, step.to_id, step.act_id),
        (Some(3), Some(10), Some(10), Some(CARRIER))
    );
    assert_eq!(
        children(step)
            .map(|child| (child.act_id, child.from_id, child.to_id))
            .collect::<Vec<_>>(),
        vec![(Some(TEMPERING), Some(10), Some(target_uid)); count],
    );
}

fn shield_hits(result: &DrainResult) -> Vec<i64> {
    result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::Hit(hit)
                if hit.shield_absorbed > 0
                    && hit.damage_from == crate::engine::manager::hp::HurtDamageFromType::Skill =>
            {
                Some(hit.source_uid)
            }
            _ => None,
        })
        .collect()
}

fn completed_actions(result: &DrainResult) -> Vec<(i64, i32)> {
    result
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::AllyAction(action) => Some((action.source_uid, action.skill_id)),
            _ => None,
        })
        .collect()
}

#[test]
fn shield_counter_releases_five_at_the_nested_attack_completion() {
    let (mut managers, pool, catalog) = setup(Vec::new());
    let result = execute(
        &mut managers,
        &pool,
        &catalog,
        [cast(-3, FIREPOWER, SkillExecutionMode::Active)],
    );
    assert_eq!(shield_hits(&result), [-3, -3, -3, -5, -5]);
    assert_eq!(
        completed_actions(&result),
        [
            (-5, TRENCH_DEFENSE),
            (10, TEMPERING),
            (10, TEMPERING),
            (10, TEMPERING),
            (10, TEMPERING),
            (10, TEMPERING),
            (-3, FIREPOWER),
        ]
    );

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(
        steps_in(&steps)
            .iter()
            .filter(|step| step.act_id == Some(CARRIER))
            .count(),
        1
    );
    let outer = steps
        .iter()
        .find(|step| step.act_id == Some(FIREPOWER))
        .unwrap();
    let driven = outer
        .act_effect
        .last()
        .unwrap()
        .fight_step
        .as_ref()
        .unwrap();
    assert_eq!(driven.act_id, Some(TRENCH_DEFENSE));
    let group = driven
        .act_effect
        .last()
        .unwrap()
        .fight_step
        .as_ref()
        .unwrap();
    assert_group(group, -5, 5);
}

#[test]
fn shield_counter_cannot_be_stolen_by_an_attack_started_after_completion() {
    let (mut managers, pool, mut catalog) = setup(vec![500]);
    catalog.insert(super::nested_casts::direct_use_passive(
        500,
        212,
        101,
        TRENCH_DEFENSE,
    ));

    let result = execute(
        &mut managers,
        &pool,
        &catalog,
        [cast(-3, TRENCH_DEFENSE, SkillExecutionMode::Active)],
    );
    assert_eq!(shield_hits(&result), [-3, -3, -5, -5]);
    assert_eq!(
        completed_actions(&result),
        [
            (-3, TRENCH_DEFENSE),
            (-5, TRENCH_DEFENSE),
            (10, TEMPERING),
            (10, TEMPERING),
            (10, TEMPERING),
            (10, TEMPERING),
        ]
    );
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let all = steps_in(&steps);
    let groups = all
        .iter()
        .copied()
        .filter(|step| step.act_id == Some(CARRIER))
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 2);
    let outer = all
        .iter()
        .find(|step| step.act_id == Some(TRENCH_DEFENSE) && step.from_id == Some(-3))
        .unwrap();
    let reaction = all
        .iter()
        .find(|step| step.act_id == Some(TRENCH_DEFENSE) && step.from_id == Some(-5))
        .unwrap();
    assert_group(children(reaction).last().unwrap(), -5, 2);
    assert_group(children(outer).last().unwrap(), -3, 2);
}

#[test]
fn shield_counter_ignores_a_standalone_skill_without_a_completion() {
    let (mut managers, pool, catalog) = setup(Vec::new());
    let result = execute(
        &mut managers,
        &pool,
        &catalog,
        [
            cast(-5, TRENCH_DEFENSE, SkillExecutionMode::Nested),
            cast(-3, TRENCH_DEFENSE, SkillExecutionMode::Active),
        ],
    );
    assert_eq!(shield_hits(&result), [-5, -5, -3, -3]);
    assert_eq!(
        completed_actions(&result),
        [(-3, TRENCH_DEFENSE), (10, TEMPERING), (10, TEMPERING)]
    );
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let all = steps_in(&steps);
    let groups = all
        .iter()
        .copied()
        .filter(|step| step.act_id == Some(CARRIER))
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 1);
    assert_group(groups[0], -3, 2);
}

#[test]
fn canceled_shield_counter_chain_consumer_cannot_claim_its_hits() {
    use crate::engine::manager::hp::{
        DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData,
    };
    let (mut managers, _, _) = setup(Vec::new());
    let hit = managers
        .execute_rule_hp(HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -3,
            target_uid: 11,
            amount: 100,
            config_effect: 0,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: -3,
                damage_from: HurtDamageFromType::Skill,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: TRENCH_DEFENSE,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap();
    let mut state = DrainState::new(TargetContext::default());
    let path = vec![0];
    let action = ActionEvent {
        source_uid: -3,
        skill_id: TRENCH_DEFENSE,
        is_attack: true,
        ..Default::default()
    };
    assert_eq!(hit.shield_counter_credits.len(), 1);
    state.open_action(path.clone());
    state
        .shield_counter
        .earn(path.clone(), hit.shield_counter_credits.into_iter());

    state.cancel_action(&path);
    assert!(state.action_chain(&path).is_none());
    assert!(
        state
            .shield_counter
            .claim(&path, &path, &action, &managers)
            .is_empty()
    );
}

#[test]
fn grouped_shield_counter_cast_skips_a_later_dead_summon() {
    let (mut managers, pool, catalog) = setup(Vec::new());
    let source_uid = 99;
    managers.register_entity(&FightEntityInfo {
        uid: Some(source_uid),
        team_type: Some(1),
        position: Some(4),
        current_hp: Some(0),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert!(pool.entity(source_uid).is_none());
    let owner = FrameOwner::BuffAct {
        owner_uid: source_uid,
        source_uid,
        buff_uid: 100,
        buff_id: CARRIER,
        key: DefinitionKey::new(802, "BeatBackByCounter"),
    };
    let mut frames = Vec::new();
    let path = push_root(&mut frames, owner.clone(), FrameTrigger::Active);
    let invocation = SkillRequest {
        source_uid,
        skill_id: TEMPERING,
    }
    .into();
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Skill(invocation),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: Some(Rc::new(RefCell::new(Some(path)))),
        independent_parent_group: None,
        frame_owner: Some(owner),
        subscriber: None,
        caster_frame: None,
    }]);
    let result = drain_queue_with_frames(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap();
    assert!(result.events.is_empty());
}

#[test]
fn shield_counter_needs_the_carrier_holder_shield() {
    let (mut managers, pool, catalog) = setup_with_shield(Vec::new(), KIPERINA_SHIELD, 13);
    let result = execute(
        &mut managers,
        &pool,
        &catalog,
        [cast(-3, FIREPOWER, SkillExecutionMode::Active)],
    );
    assert_eq!(shield_hits(&result), [-3, -3, -3, -5, -5]);
    assert_eq!(
        completed_actions(&result),
        [(-5, TRENCH_DEFENSE), (-3, FIREPOWER)]
    );
}
