use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{self, classify::BehaviorKind, registry},
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn spends_the_target_mark_before_applying_either_configured_enhancement() {
    crate::test_support::init_config();
    let buff = |id, uid, layers| BuffInfo {
        buff_id: Some(id),
        uid: Some(uid),
        from_uid: Some(10),
        layer: Some(layers),
        ..Default::default()
    };
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![buff(31320120, 80, 2), buff(31320121, 81, 3)],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31320166, 31320168], []);
    let run = |fight: &Fight, skill_id| {
        let mut managers = BattleManagers::seeded(fight);
        let slot = catalog
            .get(skill_id)
            .unwrap()
            .slots
            .iter()
            .find(|slot| slot.behavior.spec.kind == BehaviorKind::EnhanceSkill)
            .unwrap();
        let mut modifiers = SkillModifiers::default();
        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: skill_id,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &TargetPool::from_fight(fight),
                determinism: &mut RoundDeterminism::default(),
                modifiers: &mut modifiers,
                target: &mut TargetContext::default(),
            },
            &slot.behavior,
        )
        .unwrap();
        for op in ops {
            let RuleOp::Command(BattleCommand::Buff(command)) = op else {
                panic!("expected a buff transaction");
            };
            managers.execute_buff(command).unwrap();
        }
        (modifiers, managers)
    };

    let (modifiers, _) = run(&fight, 31320168);
    assert!(modifiers.rates.is_empty());
    assert!(modifiers.attack_attributes.is_empty());

    fight.defender.as_mut().unwrap().entitys[0]
        .buffs
        .push(buff(31320128, 82, 2));
    let (modifiers, managers) = run(&fight, 31320168);
    assert_eq!(managers.buff.buff_id_amount(-1, 31320128), 1);
    assert_eq!(managers.buff.buff_id_amount(10, 31320120), 2);
    assert_eq!(
        modifiers.rates,
        [SkillRateModifier::fixed(-1, 60229, 400, true)]
    );
    assert_eq!(
        modifiers.attack_attributes,
        [(AttrId::IncantationSkillUltMightMultiplier, 15)]
    );

    let (modifiers, managers) = run(&fight, 31320166);
    assert_eq!(managers.buff.buff_id_amount(-1, 31320128), 1);
    assert_eq!(managers.buff.buff_id_amount(10, 31320120), 5);
    assert!(modifiers.rates.is_empty());
    assert!(modifiers.attack_attributes.is_empty());
}

#[test]
fn rejects_empty_list_entries_and_missing_buff_definitions() {
    crate::test_support::init_config();
    let game = config::configs::get();
    let catalog = SkillEffectCatalog::from_roots(game, [31320168], []);
    let mut behavior = catalog
        .get(31320168)
        .unwrap()
        .slots
        .iter()
        .find(|slot| slot.behavior.spec.kind == BehaviorKind::EnhanceSkill)
        .unwrap()
        .behavior
        .clone();
    let definition = registry::find(&behavior).unwrap();
    behavior.raw_args[4] = "31320120,,31320121".into();
    assert!(!(definition.supports.unwrap())(&behavior));
    behavior.raw_args[4] = "31320120,2147483647".into();
    assert_eq!(
        (definition.validate_config)(game, &behavior),
        Err(BehaviorConfigError::MissingBuff)
    );
}
