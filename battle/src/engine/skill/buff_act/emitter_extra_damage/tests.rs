use sonettobuf::{
    BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, effect_type_enum::EffectType,
};

use super::*;
use crate::engine::{
    event::{
        kind::EventKind,
        subscription::{ReactionTiming, SubscriptionKey},
    },
    manager::BattleManagers,
    runtime::{
        change::BattleChange,
        determinism::RoundDeterminism,
        drain::{self, ReactionLane},
        record::{FrameItem, FrameOwner},
    },
    skill::{
        action::{SkillActionEvent, SkillExecutionMode, SkillPhase},
        buff_act::registry::{
            BuffActDestination, RuntimeActorScope, RuntimeFrameSource, destination, find,
        },
        effect::SkillEffectCatalog,
        rule::DefinitionKey,
        target::{TargetContext, TargetPool},
    },
};

fn subscriber() -> BuffActSubscriber {
    let key = DefinitionKey::new(1087, "EmitterExtraDamage");
    BuffActSubscriber {
        owner_uid: -5,
        source_uid: 10,
        buff_uid: 2094,
        buff_id: 30530501,
        team_type: 2,
        owner_alive: true,
        amount: 3,
        key: SubscriptionKey::at_phase(EventKind::SkillAction, key, Some(SkillPhase::HitPassives)),
        act_type: "EmitterExtraDamage".to_owned(),
        effect_time: 211,
        effect_condition: 0,
        args: vec![300],
        raw: "1087#300".to_owned(),
    }
}

fn impromptu() -> BattleEvent {
    BattleEvent::SkillAction(SkillActionEvent {
        source_uid: emitter::UID,
        skill_id: 31411001,
        target_uid: -7,
        target_uids: vec![-7, -5, -6],
        attacked_target_uids: vec![-7, -5, -6],
        phase: SkillPhase::HitPassives,
        skill_slot: -1,
        is_attack: true,
        rank: 0,
        skill_type: 2,
        effect_tag: 0,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1_593_048,
        kill_count: 0,
        crit_count: 3,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: SkillExecutionMode::Nested,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    })
}

#[test]
fn route_is_owned_by_the_applier_at_hit_passives() {
    let definition = find(1087, "EmitterExtraDamage").unwrap();

    assert!(supports(&[300]));
    assert!(!supports(&[0]));
    assert_eq!(
        destination(1087, "EmitterExtraDamage", &[300]),
        Some(BuffActDestination::Runtime)
    );
    assert_eq!(
        definition.runtime.event_override,
        Some(EventKind::SkillAction)
    );
    assert_eq!(
        definition.runtime.phase_override,
        Some(SkillPhase::HitPassives)
    );
    assert_eq!(definition.runtime.frame_source, RuntimeFrameSource::Applier);
    assert_eq!(
        definition.runtime.actor_scope,
        RuntimeActorScope::OpposingTeam
    );
    assert_eq!(
        definition.runtime.reaction_timing,
        ReactionTiming::Immediate
    );
}

#[test]
fn only_a_living_attacked_holder_reacts_to_emitter_damage() {
    let mut subscriber = subscriber();
    let mut event = impromptu();
    if let BattleEvent::SkillAction(action) = &mut event {
        action.attacked_target_uids = vec![-7, -6];
    }
    assert!(rule_ops(&subscriber, &event).unwrap().is_empty());

    if let BattleEvent::SkillAction(action) = &mut event {
        action.attacked_target_uids.push(-5);
    }
    subscriber.owner_alive = false;
    assert!(rule_ops(&subscriber, &event).unwrap().is_empty());

    subscriber.owner_alive = true;
    if let BattleEvent::SkillAction(action) = &mut event {
        action.source_uid = 10;
    }
    assert!(rule_ops(&subscriber, &event).unwrap().is_empty());
}

#[test]
fn drain_keeps_marker_damage_and_consumption_in_the_applier_frame() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                emitter::activation_entity(),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-5),
                team_type: Some(2),
                current_hp: Some(2_000_000),
                attr: Some(HeroAttribute {
                    hp: Some(2_000_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(2094),
                    buff_id: Some(30530501),
                    from_uid: Some(10),
                    count: Some(3),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [30530501]);
    let result = drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 5,
            ..Default::default()
        },
        impromptu(),
        ReactionLane::BuffActs,
        Some(&[-5]),
    )
    .unwrap();
    let [root] = result.frames.as_slice() else {
        panic!("expected one command frame")
    };
    let frame = root
        .items
        .iter()
        .find_map(|item| match item {
            FrameItem::Child(frame)
                if matches!(
                    frame.owner,
                    FrameOwner::BuffAct {
                        owner_uid: -5,
                        source_uid: 10,
                        buff_uid: 2094,
                        key,
                        ..
                    } if key == DefinitionKey::new(1087, "EmitterExtraDamage")
                ) =>
            {
                Some(frame)
            }
            _ => None,
        })
        .expect("expected one Neuralgia subscriber frame");
    assert!(matches!(
        frame.items.as_slice(),
        [
            FrameItem::Change(marker),
            FrameItem::Change(hp),
            FrameItem::Change(buff)
        ] if matches!(marker.as_ref(), BattleChange::EffectMarker(marker)
                if marker.effect_type == EffectType::Emitterextradamage as i32
                    && marker.effect_num == 2094
                    && marker.effect_num1 == Some(477_914))
            && matches!(hp.as_ref(), BattleChange::Hp(changes)
                if matches!(changes.hp.as_ref(), Some(change)
                    if change.delta == -477_914
                        && matches!(change.hurt.as_ref(), Some(hurt)
                            if hurt.display_amount == Some(477_914)
                                && hurt.reduce_hp == -477_914)))
            && matches!(buff.as_ref(), BattleChange::Buff(changes)
                if matches!(changes.change.refreshed.as_slice(), [update]
                    if update.source_uid == 10
                        && update.target_uid == -5
                        && update.before.count == Some(3)
                        && update.after.count == Some(2)))
    ));
    assert_eq!(managers.hp.current(-5), 1_522_086);
    assert_eq!(managers.buff.buff_id_amount(-5, 30530501), 2);
}
