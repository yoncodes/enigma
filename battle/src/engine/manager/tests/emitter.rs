use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute};

use crate::engine::{
    entity::attr::AttrId,
    manager::{BattleManagers, emitter},
    skill::target::TargetPool,
};

fn ally(uid: i64, attack: i32, buffs: Vec<BuffInfo>) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1_000),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            attack: Some(attack),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    }
}

#[test]
fn emitter_averages_complete_ally_attributes_before_division() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                ally(
                    10,
                    1_000,
                    vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(101),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                ),
                ally(
                    11,
                    1_000,
                    vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31_410_407),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                ),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(
        emitter::average_ally_fixed_attribute(&managers.emitter, &managers.buff, AttrId::Attack),
        78
    );

    managers.attribute.override_ex(
        10,
        &HeroExAttribute {
            cri: Some(1),
            ..Default::default()
        },
    );
    managers.attribute.sync_emitter_average(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31_080_145, 0);
    assert_eq!(
        emitter::average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::CriticalRate,
        ),
        38
    );
}

#[test]
fn emitter_keeps_all_allies_in_the_average_after_a_defeat() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                ally(10, 1_000, Vec::new()),
                ally(
                    11,
                    3_000,
                    vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(101),
                        from_uid: Some(11),
                        ..Default::default()
                    }],
                ),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers.hp.lose(11, 1_000, 0);
    let runtime_pool = pool.runtime_view(&managers);

    assert_eq!(pool.entity(emitter::UID).unwrap().attack, 2_000);
    assert_eq!(runtime_pool.main_allies(emitter::UID).len(), 1);
    assert_eq!(
        emitter::average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::Attack,
        ),
        25
    );
}

#[test]
fn emitter_attribute_delta_clamps_before_narrowing() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![ally(10, 1_000, Vec::new()), ally(11, 1_000, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    for uid in [10, 11] {
        managers.attribute.override_ex(
            uid,
            &HeroExAttribute {
                cri: Some(i32::MAX),
                ..Default::default()
            },
        );
    }
    managers.attribute.override_ex(
        emitter::UID,
        &HeroExAttribute {
            cri: Some(i32::MIN),
            ..Default::default()
        },
    );

    assert_eq!(
        emitter::average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::CriticalRate,
        ),
        i32::MAX
    );
}
