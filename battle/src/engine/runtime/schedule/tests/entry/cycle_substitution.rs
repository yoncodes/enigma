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
