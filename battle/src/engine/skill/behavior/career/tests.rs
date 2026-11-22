use super::*;

#[test]
fn career_ratio_fix_keeps_signed_permille_values() {
    assert!(Handler::supports(&ParsedBehavior::new(
        60058,
        "CareerRatioFix",
        vec![300],
    )));
    assert!(Handler::supports(&ParsedBehavior::new(
        60058,
        "CareerRatioFix",
        vec![-600],
    )));
}

#[test]
fn attack_career_accepts_only_real_afflatuses() {
    assert!(Handler::supports(&ParsedBehavior::new(
        100036,
        "SkillChangeAttackCareer",
        vec![1],
    )));
    assert!(!Handler::supports(&ParsedBehavior::new(
        100036,
        "SkillChangeAttackCareer",
        vec![101],
    )));
}

#[test]
fn career_change_emits_entity_owned_state() {
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: -1,
            source_team: 2,
            target_uid: -1,
            active_skill_id: 20,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::new(50013, "ChangeCareer", vec![3]),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Entity(EntityCommand {
            target_uid: -1,
            operation: EntityOperation::ChangeCareer { career: 3 },
            ..
        }))]
    ));
}

#[test]
fn attack_career_is_written_to_the_skill_modifier() {
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    assert!(Handler::collect_attack_modifier(
        AttackModifierContext {
            operation: BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 20,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            owner_skill_id: 0,
            conditions: &[],
        },
        &ParsedBehavior::new(100036, "SkillChangeAttackCareer", vec![1]),
    ));
    assert_eq!(modifiers.attack_career, Some(1));
}

#[test]
fn active_skill_attack_career_uses_the_same_modifier_path() {
    let managers = crate::engine::manager::BattleManagers::default();
    let pool = crate::engine::skill::target::TargetPool::default();
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    assert_eq!(
        Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 20,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &ParsedBehavior::new(100036, "SkillChangeAttackCareer", vec![1]),
        ),
        Some(Vec::new())
    );
    assert_eq!(modifiers.attack_career, Some(1));
}
