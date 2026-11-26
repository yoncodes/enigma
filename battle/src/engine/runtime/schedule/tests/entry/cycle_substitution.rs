use super::*;

const PASSIVE: i32 = 116_320_000;

fn entity(uid: i64, position: i32, buffed: bool) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        model_id: Some(116_320_003),
        team_type: Some(2),
        current_hp: Some(100),
        position: Some(position),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        passive_skill: vec![PASSIVE],
        buffs: buffed
            .then(|| BuffInfo {
                uid: Some(uid.unsigned_abs() as i64),
                buff_id: Some(PASSIVE),
                from_uid: Some(uid),
                ..Default::default()
            })
            .into_iter()
            .collect(),
        ..Default::default()
    }
}

fn collect_roster_effects(
    step: &sonettobuf::FightStep,
    effects: &mut Vec<sonettobuf::effect_type_enum::EffectType>,
) {
    for effect in &step.act_effect {
        if let Some(kind) = effect
            .effect_type
            .and_then(|value| sonettobuf::effect_type_enum::EffectType::try_from(value).ok())
            && matches!(
                kind,
                sonettobuf::effect_type_enum::EffectType::Changehero
                    | sonettobuf::effect_type_enum::EffectType::Callmonstertosub
                    | sonettobuf::effect_type_enum::EffectType::Removemonstersub
            )
        {
            effects.push(kind);
        }
        if let Some(child) = &effect.fight_step {
            collect_roster_effects(child, effects);
        }
    }
}

fn collect_cycle_grants(frame: &SemanticFrame, owner: i64, grants: &mut Vec<()>) {
    for item in &frame.items {
        match item {
            FrameItem::Change(change)
                if matches!(change.as_ref(), BattleChange::Buff(change)
                if change.change.added.as_ref().is_some_and(|added|
                    added.source_uid == owner && added.target_uid == owner
                    && added.buff.buff_id == Some(PASSIVE))) =>
            {
                grants.push(())
            }
            FrameItem::Child(child) => collect_cycle_grants(child, owner, grants),
            _ => {}
        }
    }
}

#[test]
fn a_cycle_reserve_enters_after_the_action_without_promoting_ordinary_deaths() {
    use crate::engine::{
        fight::rules::ATTACKER_SIDE_UID,
        manager::{
            buff::{BuffCommand, BuffGrant},
            entity::{EntityCommand, EntityOperation},
        },
        skill::action::{SkillInvocation, SkillRequest, SkillTarget},
    };
    use sonettobuf::effect_type_enum::EffectType;

    init_config();
    let limbo_entity = |uid, position, cycle: bool| {
        let mut entity = entity(uid, position, cycle);
        entity.model_id = Some(120384122);
        entity.passive_skill = vec![9260001];
        for buff in &mut entity.buffs {
            buff.buff_id = Some(9260001);
        }
        entity
    };
    let run = |terminal| {
        let mut ordinary = limbo_entity(-1, 1, false);
        ordinary.model_id = Some(120384121);
        let fight = Fight {
            battle_id: Some(120384100).filter(|_| terminal),
            version: Some(7),
            progress: Some(0),
            progress_max: Some(100),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    position: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![ordinary, limbo_entity(-2, 2, true)],
                sub_entitys: vec![limbo_entity(-5, -1, false), limbo_entity(-6, -2, false)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(1, "AddBuff"),
                },
                source_uid: ATTACKER_SIDE_UID,
                target_uid: ATTACKER_SIDE_UID,
                buff_id: 9260101,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let mut catalog = SkillEffectCatalog::from_fight(crate::test_support::game_data(), &fight);
        catalog.insert(ParsedSkillEffect {
            skill_id: 900,
            slots: vec![SkillEffectSlot::new(
                ParsedBehavior::new(60015, "Kill", Vec::new()),
                TargetRequest {
                    code: 202,
                    raw: Vec::new(),
                },
            )],
        });
        let mut skill: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 900,
        }
        .into();
        skill.target = SkillTarget::Explicit(-2);

        let result = run_active_action(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                battle_id: fight.battle_id.unwrap_or_default(),
                ..Default::default()
            },
            ActiveActionRequest {
                skill,
                grants_ex_point: false,
                grant_after_action: false,
                composition_rewards: Vec::new(),
                prelude: Vec::new(),
            },
        )
        .unwrap();

        (fight, managers, result)
    };

    let (mut fight, mut managers, mut result) = run(false);
    let effects = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(managers.hp.current(-2), 0);
    assert_eq!(managers.entity.roster_lane(-1), Some(RosterLane::Main));
    assert_eq!(managers.entity.roster_lane(-2), Some(RosterLane::Replaced));
    assert_eq!(managers.entity.roster_lane(-5), Some(RosterLane::Main));
    assert_eq!(managers.entity_snapshot(-5).unwrap().position, Some(2));
    assert_eq!(managers.entity.roster_lane(-6), Some(RosterLane::Reserve));
    assert!(managers.buff.has_active_buff_id(-5, 9260103));
    assert!(!managers.buff.has_buff_id(-6, 9260103));
    let split = effects.last().unwrap();
    assert_eq!(
        split.act_effect[0].effect_type,
        Some(EffectType::Splitstart as i32)
    );
    assert_eq!(
        split.act_effect.last().unwrap().effect_type,
        Some(EffectType::Splitend as i32)
    );

    managers.hp.lose(-1, 100, 10);
    let summoned = managers
        .execute_entity(EntityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60008, "Summon"),
            },
            source_uid: -5,
            target_uid: -5,
            operation: EntityOperation::SummonCombatant {
                model_id: 30111003,
                position: 1,
            },
        })
        .unwrap()
        .entity
        .uid
        .unwrap();
    assert_eq!(managers.entity.roster_lane(-1), Some(RosterLane::Replaced));

    let mut pool = TargetPool::from_fight(&fight);
    result.frames.clear();
    sync_cycle_roster(&mut fight, &mut managers, &mut pool, &result);
    assert!(pool.defender_main.iter().any(|entity| entity.uid == -5));
    assert!(!pool.defender_main.iter().any(|entity| entity.uid == -2));
    let halo = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: summoned,
            target_uid: summoned,
            buff_id: 109320111,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let recipients = halo
        .fanout
        .iter()
        .flat_map(|fanout| fanout.added.iter().map(|added| added.target_uid))
        .collect::<Vec<_>>();
    assert_eq!(recipients, [-5]);

    let catalog = SkillEffectCatalog::from_fight(crate::test_support::game_data(), &fight);
    run_ai_actions(
        &fight,
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [AiSkillChoice {
            source_uid: -2,
            skill_id: 100,
            target_uid: 10,
        }],
        &[],
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(-5), 0);
    assert_eq!(managers.ex_point.get(-2), 0);

    let (_, managers, result) = run(true);
    let effects = crate::engine::packet::timeline::project(&result.frames).unwrap();
    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.hp.current(-2), 0);
    assert_eq!(managers.entity.roster_lane(-2), Some(RosterLane::Main));
    assert_eq!(managers.entity.roster_lane(-5), Some(RosterLane::Reserve));
    assert!(
        !effects
            .iter()
            .flat_map(|step| &step.act_effect)
            .any(|effect| effect.effect_type == Some(EffectType::Splitstart as i32))
    );
}

#[test]
fn cycle_to_sub_repeats_through_entering_reserves_in_capture_order() {
    init_config();
    let mut fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                entity(-2, 1, true),
                entity(-3, 2, true),
                entity(-4, 3, true),
            ],
            sub_entitys: vec![entity(-5, -1, false), entity(-6, -2, false)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    let battle_catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let catalog = SkillEffectCatalog::from_fight(crate::test_support::game_data(), &fight);
    let mut managers = BattleManagers::seeded_with_catalog(battle_catalog, &fight).unwrap();
    managers.battle_rule.enable_immediate_reserve_promotion(
        crate::engine::manager::battle_rule::ImmediateReservePromotionCommand { team_type: 2 },
    );
    let mut determinism = RoundDeterminism::default();
    let mut frames = Vec::new();

    for batch_index in 0..4 {
        for uid in managers.entity.alive_combatants(2, &managers.hp) {
            managers.hp.lose(uid, 100, 0);
        }
        let promotions = managers.promote_immediate_reserves();
        assert_eq!(promotions.len(), 3);
        for promotion in promotions.promotions() {
            assert_eq!(
                managers.entity.roster_lane(promotion.defeated_uid),
                Some(RosterLane::RetiringReserve)
            );
            assert_eq!(
                promotion.queued_reserve.as_ref().unwrap().entity.position,
                Some(-2)
            );
        }
        let first_entry = promotions.promotions()[0].entering_uid;
        if batch_index == 0 {
            assert!(!managers.buff.has_active_buff_id(first_entry, PASSIVE));
        }
        let pool = TargetPool::from_fight_with_catalog(battle_catalog, &fight);
        let mut result = run_promotions(
            &pool,
            &mut managers,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            &promotions,
        )
        .unwrap();
        let entered: Vec<_> = result
            .events
            .iter()
            .filter_map(|event| {
                if let BattleEvent::EntityEntered {
                    target_uid,
                    entry: EntityEntry::Reserve,
                } = event
                {
                    Some(*target_uid)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            entered,
            promotions
                .promotions()
                .iter()
                .map(|promotion| promotion.entering_uid)
                .collect::<Vec<_>>()
        );
        if batch_index == 0 {
            let frame = &result.frames[0];
            let promotion = frame
                .items
                .iter()
                .position(|item| {
                    matches!(item, FrameItem::Change(change)
                    if matches!(change.as_ref(), BattleChange::EntityPromotion(record)
                        if record.entering_uid == first_entry))
                })
                .unwrap();
            let mut grants = Vec::new();
            for item in &frame.items[promotion + 1..] {
                if let FrameItem::Child(setup) = item {
                    collect_cycle_grants(setup, first_entry, &mut grants);
                }
            }
            assert_eq!(grants.len(), 1);
        }
        for promotion in promotions.promotions() {
            assert_eq!(
                managers.entity.roster_lane(promotion.defeated_uid),
                Some(RosterLane::Replaced)
            );
        }
        frames.append(&mut result.frames);
        for uid in managers.entity.alive_combatants(2, &managers.hp) {
            assert!(managers.buff.has_buff_act_kind(
                uid,
                crate::engine::skill::buff_act::registry::BuffActKind::CycleToSub,
            ));
        }
        managers.sync_entities(&mut fight);
        assert_eq!(
            fight
                .defender
                .as_ref()
                .unwrap()
                .sub_entitys
                .iter()
                .map(|entity| entity.position)
                .collect::<Vec<_>>(),
            [Some(-1), Some(-2)]
        );
    }

    let mut effects = Vec::new();
    for step in crate::engine::packet::timeline::project(&frames).unwrap() {
        collect_roster_effects(&step, &mut effects);
    }
    let one_batch = [
        sonettobuf::effect_type_enum::EffectType::Changehero,
        sonettobuf::effect_type_enum::EffectType::Callmonstertosub,
        sonettobuf::effect_type_enum::EffectType::Changehero,
        sonettobuf::effect_type_enum::EffectType::Callmonstertosub,
        sonettobuf::effect_type_enum::EffectType::Changehero,
        sonettobuf::effect_type_enum::EffectType::Callmonstertosub,
        sonettobuf::effect_type_enum::EffectType::Removemonstersub,
        sonettobuf::effect_type_enum::EffectType::Removemonstersub,
        sonettobuf::effect_type_enum::EffectType::Removemonstersub,
    ];
    assert_eq!(effects, one_batch.repeat(4));
}
