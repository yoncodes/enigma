use super::*;

#[test]
fn shared_damage_reaction_transfers_the_number_of_share_recipients() {
    crate::test_support::init_config();
    let entity = |uid, passive_skill| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, vec![434111]), entity(11, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::DamageShared {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(872, "ShareHurt"),
            },
            source_uid: -1,
            target_uid: 10,
            participant_uids: vec![10, 11, 12],
            amount: 300,
            share_count: 3,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
        },
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 434121), 3);
    assert_eq!(managers.buff.buff_id_amount(11, 434121), 3);

    let mut attacked_managers = BattleManagers::seeded(&fight);
    run_event(
        &mut attacked_managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Hit(crate::engine::event::payload::HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            skill_id: 1,
            amount: 100,
            resolved_damage: 100,
            shield_absorbed: 0,
            career_restraint: false,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            share_count: 3,
            assassinate: false,
            ignore_riposte: false,
        }),
    )
    .unwrap();
    assert_eq!(attacked_managers.buff.buff_id_amount(10, 434121), 3);
    assert_eq!(attacked_managers.buff.buff_id_amount(11, 434121), 3);

    let mut genesis_managers = BattleManagers::seeded(&fight);
    run_event(
        &mut genesis_managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::DamageShared {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(30014, "OriginDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            participant_uids: vec![10, 11, 12],
            amount: 300,
            share_count: 3,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::SkillEffect,
        },
    )
    .unwrap();
    assert_eq!(genesis_managers.buff.buff_id_amount(10, 434121), 0);
    assert_eq!(genesis_managers.buff.buff_id_amount(11, 434121), 0);
}

#[test]
fn shared_damage_batch_counts_each_attack_damage_lane_once() {
    let event = |damage_from| BattleEvent::DamageShared {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(872, "ShareHurt"),
        },
        source_uid: -1,
        target_uid: 10,
        participant_uids: vec![10, 11, 12],
        amount: 300,
        share_count: 3,
        damage_from,
    };
    let genesis = [
        event(crate::engine::manager::hp::HurtDamageFromType::SkillEffect),
        event(crate::engine::manager::hp::HurtDamageFromType::SkillEffect),
    ];
    let skill = [
        event(crate::engine::manager::hp::HurtDamageFromType::Skill),
        event(crate::engine::manager::hp::HurtDamageFromType::Skill),
    ];
    let mut state = DrainState::default();

    assert_eq!(
        state.record_action_shared_damage(Some(&[1]), &genesis),
        std::collections::HashMap::from([(10, 3), (11, 3), (12, 3)])
    );
    assert_eq!(
        state.record_action_shared_damage(Some(&[1]), &skill),
        std::collections::HashMap::from([(10, 6), (11, 6), (12, 6)])
    );
}
