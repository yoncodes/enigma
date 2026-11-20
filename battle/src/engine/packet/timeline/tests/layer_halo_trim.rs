use super::*;

type WireLine = (i32, i64, Option<(i64, i32)>);

fn wire(fanout: &crate::engine::manager::buff::BuffFanoutResult) -> Vec<WireLine> {
    project_change(
        &crate::engine::runtime::change::BattleChange::BuffFanout(Box::new(fanout.clone())),
        true,
        HurtInfoWireLayout::Version7,
        AbsorbHurtMapLayout::default(),
        RedealWireLayout::Version7,
    )
    .unwrap()
    .into_iter()
    .map(|effect| {
        (
            effect.effect_type.unwrap_or_default(),
            effect.target_id.unwrap_or_default(),
            effect.buff.and_then(|buff| buff.uid.zip(buff.layer)),
        )
    })
    .collect()
}

#[test]
fn each_layered_halo_stack_keeps_its_own_copies_and_trims_them_per_ally() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10, 11, 12]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let grant = |amount| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31460002,
            amount,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let copy_uids = |change: &crate::engine::manager::buff::BuffChanges| {
        change.fanout[0]
            .added
            .iter()
            .map(|added| added.buff.uid.unwrap())
            .collect::<Vec<_>>()
    };
    let first = managers.buff.execute(&managers.hp, grant(None)).unwrap();
    let first_copies = copy_uids(&first);
    managers.buff.advance_durations(103);
    let second = managers.buff.execute(&managers.hp, grant(None)).unwrap();
    let second_copies = copy_uids(&second);
    assert_eq!(second_copies.len(), 2);

    let merged = managers.buff.execute(&managers.hp, grant(Some(4))).unwrap();
    assert_eq!(merged.fanout.len(), 1);
    let mut expected = Vec::new();
    for (index, target) in [11, 12].into_iter().enumerate() {
        expected.extend([
            (
                EffectType::Buffdel as i32,
                target,
                Some((first_copies[index], 1)),
            ),
            (
                EffectType::Buffupdate as i32,
                target,
                Some((second_copies[index], 5)),
            ),
            (EffectType::Layerslavehalo as i32, target, None),
            (EffectType::Attr as i32, target, None),
        ]);
    }
    assert_eq!(wire(&merged.fanout[0]), expected);

    managers.buff.advance_durations(103);
    let third = managers.buff.execute(&managers.hp, grant(None)).unwrap();
    assert_eq!(third.fanout.len(), 1);
    let allies = &third.fanout[0].allies;
    assert_eq!(
        allies
            .iter()
            .map(|ally| ally.target_uid)
            .collect::<Vec<_>>(),
        [11, 12]
    );
    let mut expected = Vec::new();
    for (index, ally) in allies.iter().enumerate() {
        let added = ally.added.as_ref().unwrap();
        assert!(added.effective);
        expected.extend([
            (
                EffectType::Buffupdate as i32,
                ally.target_uid,
                Some((second_copies[index], 4)),
            ),
            (
                EffectType::Buffadd as i32,
                ally.target_uid,
                Some((added.buff.uid.unwrap(), 1)),
            ),
            (EffectType::Layerslavehalo as i32, ally.target_uid, None),
            (EffectType::Attr as i32, ally.target_uid, None),
        ]);
    }
    assert_eq!(wire(&third.fanout[0]), expected);
    let copy_events = third
        .events()
        .into_iter()
        .filter_map(|event| match event {
            crate::engine::event::payload::BattleEvent::BuffChanged(change)
                if change.target_uid != 10 =>
            {
                Some(("changed", change.target_uid, change.buff_uid))
            }
            crate::engine::event::payload::BattleEvent::BuffAdded(change)
                if change.target_uid != 10 =>
            {
                Some(("added", change.target_uid, change.buff_uid))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_events = allies
        .iter()
        .enumerate()
        .flat_map(|(index, ally)| {
            [
                ("changed", ally.target_uid, second_copies[index]),
                (
                    "added",
                    ally.target_uid,
                    ally.added.as_ref().unwrap().buff.uid.unwrap(),
                ),
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(copy_events, expected_events);
}
