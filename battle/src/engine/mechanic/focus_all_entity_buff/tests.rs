use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    packet::effect::EffectPacket,
    skill::rule::{DefinitionKey, RuleDomain},
};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60263, "FocusAllEntityBuff"),
};

fn entity(uid: i64, team_type: i32, buff_uid: i64) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        buffs: vec![BuffInfo {
            buff_id: Some(303901411),
            uid: Some(buff_uid),
            from_uid: Some(-3),
            layer: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn managers() -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(-1, 1, 1039),
                entity(-2, 1, 1040),
                entity(-3, 1, 1041),
                entity(-4, 1, 1042),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-5, 2, 1043),
                entity(-6, 2, 1044),
                entity(-7, 2, 1045),
            ],
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[test]
fn transaction_alternates_exact_removals_and_layer_refreshes() {
    let mut managers = managers();
    let changes = execute(
        &mut managers,
        FocusAllEntityBuffCommand {
            origin: ORIGIN,
            source_uid: -3,
            target_uid: -3,
            buff_id: 303901411,
            candidate_uids: vec![-1, -2, -4, -5, -6, -7],
        },
    )
    .unwrap()
    .unwrap();

    assert_eq!(changes.buffs.len(), 12);
    for (index, owner_uid) in [-1, -2, -4, -5, -6, -7].into_iter().enumerate() {
        let removal = &changes.buffs[index * 2].change;
        assert_eq!(removal.removed.len(), 1);
        assert_eq!(removal.removed[0].target_uid, owner_uid);
        assert_eq!(removal.removed[0].buff.buff_id, Some(303901411));

        let refresh = &changes.buffs[index * 2 + 1].change;
        assert_eq!(refresh.refreshed.len(), 1);
        assert_eq!(refresh.refreshed[0].target_uid, -3);
        assert_eq!(refresh.refreshed[0].after.uid, Some(1041));
        assert_eq!(refresh.refreshed[0].after.layer, Some(index as i32 + 2));
    }
    assert_eq!(managers.buff.buff_id_amount(-3, 303901411), 7);
    assert_eq!(changes.marker.target_uid, -3);
    assert_eq!(changes.marker.effect_type, 366);
    assert_eq!(changes.marker.config_effect, 60263);
    assert_eq!(changes.marker.reserve_id, Some(0));
    let reserve_str = changes.marker.reserve_str.clone().unwrap();
    let (reservations, donors) = reserve_str.split_once(',').unwrap();
    let reservation_uids = reservations.split('#').collect::<Vec<_>>();
    assert_eq!(
        reservation_uids,
        ["1046", "1047", "1048", "1049", "1050", "1051"]
    );
    assert_eq!(donors, "-1:1039|-2:1040|-4:1042|-5:1043|-6:1044|-7:1045");

    let wire = changes
        .buffs
        .iter()
        .flat_map(EffectPacket::recorded_buff_changes)
        .chain(std::iter::once(EffectPacket::effect_marker(
            changes.marker.clone(),
        )))
        .collect::<Vec<_>>();
    assert_eq!(wire.len(), 13);
    for (index, owner_uid) in [-1, -2, -4, -5, -6, -7].into_iter().enumerate() {
        let removed = &wire[index * 2];
        assert_eq!(removed.target_id, Some(owner_uid));
        assert_eq!(removed.effect_type, Some(6));
        assert_eq!(removed.buff.as_ref().and_then(|buff| buff.layer), Some(1));

        let refreshed = &wire[index * 2 + 1];
        assert_eq!(refreshed.target_id, Some(-3));
        assert_eq!(refreshed.effect_type, Some(7));
        assert_eq!(
            refreshed.buff.as_ref().and_then(|buff| buff.uid),
            Some(1041)
        );
        assert_eq!(
            refreshed.buff.as_ref().and_then(|buff| buff.layer),
            Some(index as i32 + 2)
        );
    }
    let marker = wire.last().unwrap();
    assert_eq!(marker.target_id, Some(-3));
    assert_eq!(marker.effect_type, Some(366));
    assert_eq!(marker.effect_num, Some(0));
    assert_eq!(marker.config_effect, Some(60263));
    assert_eq!(marker.reserve_id, Some(0));
    assert_eq!(marker.reserve_str.as_deref(), Some(reserve_str.as_str()));

    let following = managers
        .plan_buff(BuffCommand::Grant(BuffGrant {
            origin: ORIGIN,
            source_uid: -3,
            target_uid: -3,
            buff_id: 101,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    assert_eq!(following.added_buff_uid(), Some(1053));
}

#[test]
fn transaction_stops_at_capacity_and_ignores_source() {
    let mut managers = managers();
    for _ in 0..10 {
        managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: ORIGIN,
                source_uid: -3,
                target_uid: -3,
                buff_id: 303901411,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
    }
    assert_eq!(managers.buff.buff_id_amount(-3, 303901411), 11);

    let changes = execute(
        &mut managers,
        FocusAllEntityBuffCommand {
            origin: ORIGIN,
            source_uid: -3,
            target_uid: -3,
            buff_id: 303901411,
            candidate_uids: vec![-3, -1, -2],
        },
    )
    .unwrap()
    .unwrap();

    assert_eq!(changes.buffs.len(), 2);
    assert_eq!(managers.buff.buff_id_amount(-3, 303901411), 12);
    assert!(!managers.buff.has_buff_id(-1, 303901411));
    assert!(managers.buff.has_buff_id(-2, 303901411));
}

#[test]
fn multi_layer_donor_fails_before_any_focus_mutation() {
    let mut managers = managers();
    managers
        .execute_buff(BuffCommand::Accumulate(BuffGrant {
            origin: ORIGIN,
            source_uid: -3,
            target_uid: -1,
            buff_id: 303901411,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    assert_eq!(managers.buff.buff_id_amount(-1, 303901411), 2);

    let error = execute(
        &mut managers,
        FocusAllEntityBuffCommand {
            origin: ORIGIN,
            source_uid: -3,
            target_uid: -3,
            buff_id: 303901411,
            candidate_uids: vec![-1, -2],
        },
    )
    .unwrap_err();

    assert_eq!(
        error,
        FocusAllEntityBuffError::UnsupportedDonorAmount {
            owner_uid: -1,
            amount: 2,
        }
    );
    assert_eq!(managers.buff.buff_id_amount(-1, 303901411), 2);
    assert_eq!(managers.buff.buff_id_amount(-2, 303901411), 1);
    assert_eq!(managers.buff.buff_id_amount(-3, 303901411), 1);
}
