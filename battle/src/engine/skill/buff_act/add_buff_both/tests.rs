use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::{
        action::{SkillActionEvent, SkillExecutionMode},
        rule::DefinitionKey,
    },
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn grants_configured_buffs_to_enemy_and_allies_then_removes_carrier() {
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2), entity(-3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 22,
        buff_id: 30091120,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(EventKind::SkillCast, DefinitionKey::new(850, "AddBuffBoth")),
        act_type: "AddBuffBoth".to_owned(),
        effect_time: 208,
        effect_condition: 0,
        args: vec![300901412, 101, 30091111],
        raw: "850#300901412#101#30091111".to_owned(),
    };
    let event = BattleEvent::SkillAction(SkillActionEvent {
        source_uid: 10,
        card_index: 0,
        skill_id: 1,
        target_uid: -1,
        target_uids: vec![-1, -2, -3],
        attacked_target_uids: vec![-2, 11, -3],
        phase: SkillPhase::HitPassives,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 0,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        mode: SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    });

    let ops = rule_ops(
        &managers,
        &pool,
        &mut Default::default(),
        &subscriber,
        &event,
    )
    .unwrap();
    let grants = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) => {
                Some((grant.target_uid, grant.buff_id, grant.amount))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        grants,
        vec![
            (-2, 300901412, None),
            (-3, 300901412, None),
            (10, 30091111, None),
            (11, 30091111, None)
        ]
    );
    assert!(matches!(
        ops.last(),
        Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove {
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(22),
                ..
            }
        ))))
    ));

    let mut non_attack = event.clone();
    let BattleEvent::SkillAction(action) = &mut non_attack else {
        unreachable!()
    };
    action.is_attack = false;
    assert!(
        rule_ops(
            &managers,
            &pool,
            &mut Default::default(),
            &subscriber,
            &non_attack,
        )
        .unwrap()
        .is_empty()
    );

    let mut ally_attack = event.clone();
    let BattleEvent::SkillAction(action) = &mut ally_attack else {
        unreachable!()
    };
    action.source_uid = 11;
    assert!(
        rule_ops(
            &managers,
            &pool,
            &mut Default::default(),
            &subscriber,
            &ally_attack,
        )
        .unwrap()
        .is_empty()
    );

    let mut wrong_phase = event;
    let BattleEvent::SkillAction(action) = &mut wrong_phase else {
        unreachable!()
    };
    action.phase = SkillPhase::AfterHit;
    assert!(
        rule_ops(
            &managers,
            &pool,
            &mut Default::default(),
            &subscriber,
            &wrong_phase,
        )
        .unwrap()
        .is_empty()
    );
}
