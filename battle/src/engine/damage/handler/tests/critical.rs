use super::*;

#[test]
fn crit_chance_reads_teammate_buff_features() {
    crate::test_support::init_config();
    let mut source = entity(10, 1, 1, 1000, 100);
    source.model_id = Some(3114);
    source.entity_type = Some(1);
    source.level = Some(180);
    source.attr.as_mut().unwrap().technic = Some(456);
    source.buffs = vec![BuffInfo {
        buff_id: Some(30630112),
        layer: Some(1),
        ..Default::default()
    }];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    assert_eq!(crit_chance(10, -1, &pool, &managers), 352);
}
#[test]
fn crit_chance_reads_dynamic_shield_attributes() {
    crate::test_support::init_config();
    let mut source = entity(10, 1, 1, 1000, 100);
    source.shield_value = Some(6000);
    source.buffs = vec![BuffInfo {
        buff_id: Some(31170009),
        layer: Some(1),
        ..Default::default()
    }];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![source],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    assert_eq!(crit_chance(10, -1, &pool, &managers), 150);
}

#[test]
fn emitter_crit_chance_averages_ally_base_and_buff_rates() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1000, 100), entity(11, 1, 1, 1000, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(100),
            ..Default::default()
        },
    );
    managers.attribute.override_ex(
        11,
        &HeroExAttribute {
            cri: Some(300),
            ..Default::default()
        },
    );
    managers.attribute.sync_emitter_average(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31080145, 0);

    assert_eq!(
        crit_chance(crate::engine::manager::emitter::UID, -1, &pool, &managers,),
        237
    );
}

#[test]
fn excess_crit_conversion_uses_the_same_attack_local_rate_as_the_crit_roll() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1, 1000, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 1, 0, 100)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(1100),
            ..Default::default()
        },
    );

    assert_eq!(
        excess_crit_rate(
            10,
            -1,
            &pool,
            &managers,
            &[(crate::engine::entity::attr::AttrId::CriticalRate, 200)],
        ),
        300
    );
}
