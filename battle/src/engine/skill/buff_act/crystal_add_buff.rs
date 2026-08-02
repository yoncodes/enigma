use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        emanation::EmanationKind,
    },
    skill::{
        action::SkillPhase,
        condition::extra::skill_kind_from_is_extra,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    feature: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if action.phase != SkillPhase::AfterDamage
        || feature.key.event != crate::engine::event::kind::EventKind::SkillAction
        || !action.is_attack
    {
        return Some(Vec::new());
    }
    let [
        buff_id,
        blue_layer,
        purple_layer,
        green_rank_two,
        green_rank_three,
        ..,
    ] = feature.args.as_slice()
    else {
        return None;
    };
    if *buff_id <= 0 {
        return Some(Vec::new());
    }
    let is_extra_action = skill_kind_from_is_extra(action.extra_skill_kind)
        .is_some_and(|kind| kind.is_extra_action());
    let blue = if is_extra_action { *blue_layer } else { 0 };
    let purple = if is_extra_action { 0 } else { *purple_layer };
    let layer = managers
        .emanation
        .count(feature.source_uid, EmanationKind::Blue)
        * blue
        + managers
            .emanation
            .count(feature.source_uid, EmanationKind::Purple)
            * purple
        + managers
            .emanation
            .count(feature.source_uid, EmanationKind::Green)
            * match action.rank {
                2 => *green_rank_two,
                3 => *green_rank_three,
                _ => 0,
            };
    if layer <= 0 {
        return Some(Vec::new());
    }

    let source_uid = action.source_uid;
    let commands = action
        .target_uids
        .iter()
        .copied()
        .filter(|target_uid| *target_uid != 0)
        .map(|target_uid| {
            BuffCommand::Grant(BuffGrant {
                origin: super::command_origin(feature).expect("registered crystal buff act"),
                source_uid,
                target_uid,
                buff_id: *buff_id,
                amount: Some(layer),
                occurrences: 1,
                child_uid_reservations: 0,
            })
        })
        .collect::<Vec<_>>();
    Some(
        commands
            .into_iter()
            .map(|command| RuleOp::Command(BattleCommand::Buff(command)))
            .collect(),
    )
}

pub fn scoped_rule_ops(
    managers: &BattleManagers,
    feature: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    rule_ops(managers, feature, event).map(|ops| {
        ops.into_iter()
            .map(super::BuffActRuleOp::subscriber_from_applier)
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::{
            action::{SkillActionEvent, SkillExecutionMode},
            buff_act::{BuffActFrameSource, BuffActRuleOp},
            rule::DefinitionKey,
        },
    };

    fn fixture() -> (BattleManagers, BuffActSubscriber, BattleEvent) {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        current_hp: Some(1_000),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        current_hp: Some(1_000),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let feature = BuffActSubscriber {
            owner_uid: 11,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 1,
            owner_alive: true,
            amount: 0,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                DefinitionKey::new(1051, "CrystalAddBuff"),
            ),
            act_type: "CrystalAddBuff".to_owned(),
            effect_time: 208,
            effect_condition: 0,
            args: vec![31340001, 1, 2, 1, 2],
            raw: "1051#31340001#1#2#1#2".to_owned(),
        };
        let event = BattleEvent::SkillAction(action());
        (managers, feature, event)
    }

    fn action() -> SkillActionEvent {
        SkillActionEvent {
            source_uid: 11,
            skill_id: 100,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: SkillPhase::AfterDamage,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 100,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 1,
            mode: SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        }
    }

    #[test]
    fn crystal_link_waits_for_selection() {
        let (managers, feature, event) = fixture();

        assert_eq!(rule_ops(&managers, &feature, &event), Some(Vec::new()));
    }

    #[test]
    fn crystal_link_uses_configured_layer_and_acting_source() {
        let (mut managers, feature, event) = fixture();
        assert!(managers.emanation.select(10, 110));

        assert!(matches!(
            rule_ops(&managers, &feature, &event).as_deref(),
            Some([RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    source_uid: 11,
                    target_uid: -1,
                    buff_id: 31340001,
                    amount: Some(1),
                    ..
                }
            )))])
        ));
    }

    #[test]
    fn selected_crystals_apply_the_configured_layer_to_attacks() {
        let (mut managers, feature, _) = fixture();
        assert!(managers.emanation.select(10, 111));
        let event = |rank, extra_skill_kind| {
            BattleEvent::SkillAction(SkillActionEvent {
                rank,
                extra_skill_kind,
                ..action()
            })
        };
        let granted_layer = |event| match rule_ops(&managers, &feature, &event).unwrap().as_slice()
        {
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))] => grant.amount,
            [] => None,
            other => panic!("unexpected crystal outputs: {other:?}"),
        };

        assert_eq!(granted_layer(event(1, 0)), Some(2));
        assert_eq!(granted_layer(event(1, 1)), Some(1));
        assert_eq!(granted_layer(event(2, 0)), Some(3));
        assert_eq!(granted_layer(event(3, 1)), Some(3));
    }

    #[test]
    fn one_action_emits_ordered_per_target_buff_commands() {
        let (mut managers, feature, mut event) = fixture();
        assert!(managers.emanation.select(10, 110));
        let BattleEvent::SkillAction(action) = &mut event else {
            unreachable!()
        };
        action.target_uids = vec![-1, -2, -3];

        let outputs = rule_ops(&managers, &feature, &event).unwrap();
        assert_eq!(
            outputs
                .iter()
                .filter_map(|op| match op {
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) => {
                        Some(grant.target_uid)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![-1, -2, -3]
        );
    }

    #[test]
    fn crystal_link_ignores_attacks_without_a_selected_matching_crystal() {
        let (mut managers, feature, mut event) = fixture();
        assert!(managers.emanation.select(10, 110));
        let BattleEvent::SkillAction(action) = &mut event else {
            unreachable!()
        };
        action.source_uid = 10;
        action.extra_skill_kind = 0;

        assert_eq!(rule_ops(&managers, &feature, &event), Some(Vec::new()));
    }

    #[test]
    fn green_crystal_applies_to_an_allies_rank_two_attack() {
        let (mut managers, feature, mut event) = fixture();
        assert!(managers.emanation.select(10, 110));
        let BattleEvent::SkillAction(action) = &mut event else {
            unreachable!()
        };
        action.source_uid = 10;
        action.rank = 2;
        action.extra_skill_kind = 0;

        assert!(matches!(
            rule_ops(&managers, &feature, &event).as_deref(),
            Some([RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    amount: Some(1),
                    ..
                }
            )))])
        ));
    }

    #[test]
    fn crystal_link_frames_are_owned_by_the_force_field_applier() {
        let (mut managers, feature, event) = fixture();
        assert!(managers.emanation.select(10, 110));

        assert!(matches!(
            scoped_rule_ops(&managers, &feature, &event).as_deref(),
            Some([BuffActRuleOp {
                source: BuffActFrameSource::Applier,
                ..
            }])
        ));
    }
}
