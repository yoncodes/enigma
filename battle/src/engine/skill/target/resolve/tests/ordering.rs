use super::*;
use crate::engine::skill::target::BehaviorTargetOrder;

#[test]
fn behavior_target_order_uses_full_identity_map_capacity_and_insertion_order() {
    let uids = [
        1, 33, 234410900, 237352626, 240494381, 240494371, 2, 3, 4, 5, 6, 7, 8,
    ];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: uids
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let mut targets = vec![234410900, 237352626, 240494381, 240494371];

    TargetResolver::order_behavior_candidates(
        &mut targets,
        BehaviorTargetOrder::EntityIdentityMap,
        &managers,
    );

    assert_eq!(targets, [234410900, 240494371, 237352626, 240494381]);
    let mut short = vec![33, 1];
    TargetResolver::order_behavior_candidates(
        &mut short,
        BehaviorTargetOrder::EntityIdentityMap,
        &managers,
    );
    assert_eq!(short, [1, 33]);
}

#[test]
fn behavior_target_order_includes_promoted_reserves() {
    let entity = |uid, hp, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        position: Some(position),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 0, 1)],
            sub_entitys: vec![entity(17, 100, -1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    assert_eq!(managers.entity.promote_reserves(&managers.hp).len(), 1);
    let mut targets = vec![17, 1];
    TargetResolver::order_behavior_candidates(
        &mut targets,
        BehaviorTargetOrder::EntityIdentityMap,
        &managers,
    );
    assert_eq!(targets, [1, 17]);
}
