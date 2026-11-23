use super::*;
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn unaffordable_extra_action_keeps_the_normal_attack_lane() {
    let additional = AdditionalDamageSpec {
        formula: crate::engine::damage::DamageFormula::CreditedSourceAdditional,
        rate: 300,
        secondary_rate: 150,
        extra_rate: 600,
        extra_secondary_rate: 300,
        temp_buff_id: 0,
        remove_buff_id: 0,
        credited_source_uid: 20,
        extra_eureka_cost: 2,
        power_id: 1,
        source_count_cost: 0,
        requires_assassination: false,
    };
    let managers = BattleManagers::default();

    let uses_extra_lane = additional.uses_extra_lane(&managers, true);

    assert!(!uses_extra_lane);
    assert_eq!(additional.rate(true, uses_extra_lane), 300);
}

#[test]
fn costed_lane_includes_ripostes_without_collapsing_reinforced_skills() {
    use crate::engine::skill::condition::extra::ExtraSkillKind;

    assert!(uses_costed_lane(ExtraSkillKind::ExtraAction.id()));
    assert!(uses_costed_lane(ExtraSkillKind::FollowUp.id()));
    assert!(uses_costed_lane(ExtraSkillKind::Riposte.id()));
    assert!(!uses_costed_lane(ExtraSkillKind::Reinforced.id()));
    assert!(!uses_costed_lane(0));
}

#[test]
fn additional_damage_uses_its_exact_replacement_buff() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(20_000),
                attr: Some(HeroAttribute {
                    hp: Some(20_000),
                    ..Default::default()
                }),
                buffs: vec![
                    BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31260171),
                        from_uid: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31200171),
                        from_uid: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let spec = AdditionalDamageSpec {
        formula: crate::engine::damage::DamageFormula::AdditionalDamage,
        rate: 750,
        secondary_rate: 750,
        extra_rate: 750,
        extra_secondary_rate: 750,
        temp_buff_id: 31260171,
        remove_buff_id: 0,
        credited_source_uid: 1,
        extra_eureka_cost: 0,
        power_id: 0,
        source_count_cost: 0,
        requires_assassination: false,
    };

    assert_eq!(
        spec.attack_replacement(&managers),
        Some(super::super::AttackReplacement {
            replaced_attr: crate::engine::entity::attr::AttrId::Attack,
            source_attr: crate::engine::entity::attr::AttrId::Hp,
            amount: 4_000,
            formula: crate::engine::damage::DamageFormula::AdditionalDamage,
        })
    );
}
