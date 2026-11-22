use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

#[test]
fn configured_life_attack_rate_uses_active_buff_amount() {
    let feature = ActiveBuffFeature {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 2,
        buff_id: 3,
        amount: 2,
        team_type: 1,
        owner_alive: true,
        act_type: String::new(),
        effect_time: 0,
        effect_condition: 0,
        raw: "1025#100".to_owned(),
        values: vec![1025, 100],
    };

    assert_eq!(skill_rate_bonus(&feature), 200);
    assert!(supports(&[100]));
    assert!(!supports(&[]));
}

#[test]
fn channel_can_read_its_transient_rate_buff_without_activating_it() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::try_global().unwrap();

    assert_eq!(buff_id_skill_rate_bonus(31260161), 100);
    assert_eq!(buff_id_skill_rate_bonus(31260181), 20);
    assert_eq!(resolve_buff_id_skill_rate_bonus(catalog, 31260161), 100);
    assert_eq!(resolve_buff_id_skill_rate_bonus(catalog, -1), 0);
}

#[test]
fn configured_additional_damage_source_supplies_its_rate_multiplier() {
    crate::test_support::init_config();
    let managers = crate::engine::manager::BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(10),
                        buff_id: Some(31260151),
                        from_uid: Some(2),
                        count: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(2),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31260161),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(
        active_damage_rate_terms(1, &managers.buff, &managers.hp),
        vec![DamageRateTerm {
            opcode: 1025,
            rate: 100,
            career_scaled: true,
            composition: DamageRateComposition::ProducerMultiplier,
        }]
    );
}
