use super::*;
use crate::engine::{
    event::{payload::HitEvent, subscription::PublicationPhase},
    manager::hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
    runtime::drain::ReactionLane,
};

fn fight() -> Fight {
    let entity = |uid, buffs| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(500),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    };
    let thorny = |uid, buff_uid| {
        entity(
            uid,
            vec![BuffInfo {
                uid: Some(buff_uid),
                buff_id: Some(90021),
                from_uid: Some(uid),
                layer: Some(2),
                ..Default::default()
            }],
        )
    };
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![thorny(-1, 100), thorny(-2, 101)],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn rebound_claim_survives_each_damage_batch_and_resets_between_casts() {
    crate::test_support::init_config();
    let fight = fight();
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut state = DrainState::default();
    let mut determinism = RoundDeterminism::default();

    let mut rebound_hits = |state: &mut DrainState, path: &[usize], target_uid, damage_from| {
        let hit = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(0, "TestHit"),
            },
            source_uid: 10,
            target_uid,
            skill_id: 1,
            amount: 300,
            resolved_damage: 300,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from,
            share_count: 0,
            assassinate: false,
            ignore_riposte: false,
        });
        let reactions = dispatch_event_groups(
            &pool,
            &managers,
            &catalog,
            &mut determinism,
            state,
            &[vec![hit]],
            path,
            path,
            Some(path),
            path,
            Some((10, 1, Some(target_uid))),
            false,
            true,
            PublicationPhase::AfterPublish,
            None,
            Some(ReactionLane::BuffActs),
            None,
            None,
        )
        .unwrap();
        let damage_count = |ops: &[QueuedOp]| {
            ops.iter()
                .filter(|queued| {
                    matches!(queued.op, RuleOp::Command(BattleCommand::ReboundDamage(_)))
                })
                .count()
        };
        let deferred = damage_count(&reactions.before_after_hit_observers);
        (damage_count(&reactions.into_ordered()), deferred)
    };

    state.open_action(vec![0]);
    assert_eq!(
        rebound_hits(&mut state, &[0], -1, HurtDamageFromType::Skill),
        (0, 2)
    );
    assert_eq!(
        rebound_hits(&mut state, &[0], -1, HurtDamageFromType::SkillEffect),
        (0, 0)
    );
    state.reactions(&[0]).hit_passives_released = true;
    assert_eq!(
        rebound_hits(&mut state, &[0], -2, HurtDamageFromType::Skill),
        (2, 0)
    );
    assert_eq!(
        rebound_hits(&mut state, &[1], -1, HurtDamageFromType::Skill),
        (2, 0)
    );
}

#[test]
fn charged_rebound_is_spent_before_a_nested_attack() {
    crate::test_support::init_config();
    let mut fight = fight();
    let buff = &mut fight.defender.as_mut().unwrap().entitys[0].buffs[0];
    buff.buff_id = Some(7160005);
    buff.layer = Some(1);
    buff.count = Some(1);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut state = DrainState::default();
    let mut determinism = RoundDeterminism::default();
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let child = push_child(
        &mut frames,
        &root,
        FrameOwner::Command,
        FrameTrigger::Active,
    );
    let hit = BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(0, "TestHit"),
        },
        source_uid: 10,
        target_uid: -1,
        skill_id: 1,
        amount: 300,
        resolved_damage: 300,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    });
    let mut reflected = 0;
    for path in [root, child] {
        state.open_action(path.clone());
        let reactions = dispatch_event_groups(
            &pool,
            &managers,
            &catalog,
            &mut determinism,
            &mut state,
            &[vec![hit.clone()]],
            &path,
            &path,
            Some(&path),
            &path,
            Some((10, 1, Some(-1))),
            false,
            true,
            PublicationPhase::AfterPublish,
            None,
            Some(ReactionLane::BuffActs),
            None,
            None,
        )
        .unwrap();
        assert!(reactions.before_after_hit_observers.is_empty());
        let result = drain_queue_with_frames(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            &mut reactions.into_ordered().into(),
            frames,
        )
        .unwrap();
        reflected += result.outcomes.iter().filter(|outcome| {
            matches!(outcome, RuleOutcome::Hp(hit) if hit.changes.target_uid == 10 && hit.changes.damage.is_some())
        }).count();
        frames = result.frames;
        assert_eq!(managers.buff.snapshot(-1, 100).unwrap().count, Some(0));
    }
    assert_eq!(reflected, 1);
}

#[test]
fn an_ignored_hit_does_not_consume_the_casts_rebound() {
    crate::test_support::init_config();
    let mut fight = fight();
    fight.attacker.as_mut().unwrap().entitys[0]
        .buffs
        .push(BuffInfo {
            uid: Some(77),
            buff_id: Some(30860141),
            from_uid: Some(10),
            ..Default::default()
        });
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Skill,
        key: DefinitionKey::new(0, "TestHit"),
    };
    let damage = || {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
            origin,
            source_uid: 10,
            target_uid: -1,
            amount: 300,
            config_effect: 0,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Damage as i32,
                display_amount: None,
            },
        })))
    };
    let remove_ignore = RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
        origin,
        target_uid: 10,
        selector: BuffRemoveSelector::Uid(77),
    })));
    let mut frames = Vec::new();
    let owner = |skill_id| FrameOwner::Skill {
        source_uid: 10,
        skill_id,
        card_index: 1,
        target_uid: Some(-1),
    };
    let cast = push_root(&mut frames, owner(1), FrameTrigger::Active);
    let child = push_child(&mut frames, &cast, owner(2), FrameTrigger::Active);
    let queued = |frame: &FramePath, op| QueuedOp {
        op,
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: Some(frame.clone()),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    };
    let mut queue = std::collections::VecDeque::from([
        queued(&cast, damage()),
        queued(&cast, remove_ignore),
        queued(&cast, damage()),
        queued(&cast, damage()),
        queued(&child, damage()),
    ]);
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
    let damage_targets = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Hp(hit) if hit.changes.damage.is_some() => Some(hit.changes.target_uid),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(damage_targets, [-1, -1, 10, 10, -1, -1, 10, 10]);
    assert!(managers.hp.current(10) < 10_000);
}
