use super::*;
use crate::engine::{
    manager::hp::HurtDamageFromType,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

fn hit(damage_from: HurtDamageFromType) -> BattleEvent {
    BattleEvent::Hit(HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        },
        source_uid: 1,
        target_uid: -1,
        skill_id: 1,
        amount: 1,
        resolved_damage: 1,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

#[test]
fn only_primary_skill_damage_publishes_target_attacked() {
    assert_eq!(
        hit(HurtDamageFromType::Skill)
            .subscription_kinds()
            .collect::<Vec<_>>(),
        vec![
            EventKind::TargetAttacked,
            EventKind::BeAttacked,
            EventKind::Riposte,
        ]
    );
    assert_eq!(
        hit(HurtDamageFromType::Additional)
            .subscription_kinds()
            .collect::<Vec<_>>(),
        vec![EventKind::BeAttacked, EventKind::Riposte]
    );
}

#[test]
fn skill_cast_uses_the_hit_passives_phase_not_action_completion() {
    let action = crate::engine::skill::action::SkillActionEvent {
        source_uid: 1,
        card_index: 0,
        skill_id: 2,
        target_uid: -1,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 10,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    };

    assert_eq!(
        BattleEvent::SkillAction(action)
            .subscription_kinds()
            .collect::<Vec<_>>(),
        vec![EventKind::SkillAction, EventKind::SkillCast]
    );
    assert_eq!(
        BattleEvent::AllyAction(Default::default())
            .subscription_kinds()
            .collect::<Vec<_>>(),
        vec![EventKind::AllyAction]
    );
}
