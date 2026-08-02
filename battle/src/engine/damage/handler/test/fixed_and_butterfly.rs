use super::*;

#[test]
fn configured_attribute_damage_uses_the_targets_max_hp_as_genesis_damage() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(42, 1, 1, 1_000, 100)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                current_hp: Some(2_000),
                attr: Some(HeroAttribute {
                    hp: Some(2_000),
                    ..Default::default()
                }),
                ..entity(-1, 2, 1, 100, 100)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext {
        active_skill_id: 100,
        ..Default::default()
    };
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(10006, "Damage"),
        vec![1, AttrId::Hp.id(), 100],
        Vec::new(),
    );

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 42,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 100,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
                source_uid: 42,
                target_uid: -1,
                amount: 200,
                effect_kind: DamageEffectKind::Genesis,
                hurt: HurtInfoData {
                    skill_id: 100,
                    damage_from: HurtDamageFromType::SkillEffect,
                    ..
                },
                ..
            })))]
        )
    ));
}
#[test]
fn butterfly_damage_uses_allied_round_skill_damage_and_capped_lingering_glow() {
    use crate::engine::{
        manager::gauge::{GaugeCommand, GaugeOperation},
        mechanic::lingering_glow,
        skill::rule::{DefinitionKey, RuleDomain},
    };

    init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1), entity(2)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60282, "ButterflyDamage"),
    };
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            lingering_glow::key(1),
            GaugeOperation::Enable { max: Some(1_000) },
        ))
        .unwrap();
    managers
        .execute_gauge(GaugeCommand::new(
            origin,
            lingering_glow::key(1),
            GaugeOperation::AccumulateRawValue {
                amount: 800_000,
                stream: 60282,
            },
        ))
        .unwrap();
    let skill_damage = |source_uid, amount| {
        HpCommand::Damage(HpDamage {
            origin,
            source_uid,
            target_uid: -1,
            amount,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damage as i32,
                display_amount: None,
            },
        })
    };
    managers.execute_hp(skill_damage(1, 1_000)).unwrap();
    managers.execute_hp(skill_damage(2, 500)).unwrap();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext {
        active_skill_id: 31390118,
        ..Default::default()
    };
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60282, "ButterflyDamage"),
        vec![30, 100_000, 7, 600_000],
        Vec::new(),
    );

    assert!(matches!(
        crate::engine::skill::behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 1,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 31390118,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        ),
        Some(ops) if matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
                amount: 108,
                effect_kind: DamageEffectKind::Genesis,
                hurt: HurtInfoData {
                    damage_from: HurtDamageFromType::SkillEffect,
                    ..
                },
                ..
            })))]
        )
    ));
}
