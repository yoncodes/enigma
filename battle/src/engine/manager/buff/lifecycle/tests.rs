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

    assert!(manager.plan_action_settlements(&action()).is_empty());
}

fn action() -> SkillActionEvent {
    SkillActionEvent {
        source_uid: 10,
        card_index: 0,
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
    }
}

#[test]
fn counted_action_settlement_consumes_one_charge() {
    crate::test_support::init_config();
    for (buff_id, count, remaining_counts) in [(30631, 3, &[2, 1, 0][..]), (2130015, 0, &[0][..])] {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(buff_id),
                        from_uid: Some(10),
                        count: Some(count),
                        layer: Some(2),
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
        let hp = HpManager::default();
        for &remaining in remaining_counts {
            let settlements = manager.plan_action_settlements(&action());
            assert_eq!(settlements.len(), 1);
            let result = manager
                .execute(&hp, settlements[0].command.clone())
                .unwrap();
            if remaining > 0 {
                assert_eq!(manager.snapshot(10, 20).unwrap().count, Some(remaining));
            } else {
                assert!(!manager.has_buff_id(10, buff_id));
                assert_eq!(result.change.removed[0].buff.count, Some(0));
            }
        }
    }
}
