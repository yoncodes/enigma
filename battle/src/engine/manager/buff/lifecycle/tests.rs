use super::*;
use crate::engine::skill::action::{SkillActionEvent, SkillPhase};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn permanent_action_marker_does_not_expire() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(431231),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut manager = BuffManager::default();
    manager.seed(&fight);

    assert!(
        manager
            .plan_action_expiries(&SkillActionEvent {
                source_uid: 10,
                skill_id: 1,
                target_uid: -1,
                target_uids: vec![-1],
                attacked_target_uids: vec![-1],
                phase: SkillPhase::HitPassives,
                skill_slot: 3,
                is_attack: true,
                rank: 1,
                skill_type: 1,
                effect_tag: 1,
                assassinate: false,
                ignore_riposte: false,
                damage_amount: 0,
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
            })
            .is_empty()
    );
}
