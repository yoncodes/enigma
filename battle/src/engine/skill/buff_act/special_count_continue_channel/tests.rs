use super::*;
use crate::engine::{
    event::kind::EventKind,
    skill::{buff_act, subscriber},
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn marker_channel_emits_only_its_configured_targets() {
    crate::test_support::init_config();
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        team_type: Some(team_type),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1), entity(11, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2), entity(-2, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31070111, 0);
    managers.buff.add_special_count(10, &[31070111], 3);
    let subscriber = subscriber::for_active_buffs(&managers, EventKind::RoundStart)
        .into_iter()
        .find(|subscriber| {
            subscriber.buff_id == 31070111
                && buff_act::subscriber_is_kind(
                    subscriber,
                    buff_act::registry::BuffActKind::SpecialCountContinueChannelBuff,
                )
        })
        .unwrap();

    let ops = rule_ops(&managers, &subscriber);

    assert_eq!(ops.len(), 5);
    assert!(matches!(
        ops.first(),
        Some(RuleOp::BuffFeatureMarker { target_uid: 10, .. })
    ));
    assert_eq!(
        ops.iter()
            .filter_map(|op| match op {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(grant))) => {
                    Some(grant.target_uid)
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![-1, -2]
    );
}
