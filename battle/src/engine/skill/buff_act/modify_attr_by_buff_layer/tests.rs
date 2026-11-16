use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn damage_bonus_keeps_its_grant_value_until_replaced() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(1000),
        attr: Some(HeroAttribute {
            hp: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(4150001),
                        from_uid: Some(10),
                        layer: Some(24),
                        count: Some(1),
                        ..Default::default()
                    }],
                    ..entity(10)
                },
                entity(20),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let grant = || {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 20,
            buff_id: 30940004,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let value = |m: &BattleManagers| {
        let active = m
            .buff
            .active_features(&m.hp)
            .into_iter()
            .find(|feature| feature.owner_uid == 20 && feature.act_id() == Some(790))
            .unwrap();
        attribute_delta(&active, AttrId::DmgBonus, &m.buff)
    };
    let first = managers.buff.execute(&managers.hp, grant()).unwrap();
    let first = first.change.added.unwrap().buff;
    let uid = first.uid.unwrap();
    assert_eq!(value(&managers), 600);

    managers.buff.update(10, 30, Some(35), None, None).unwrap();
    let tick = managers.buff.update(20, uid, None, None, Some(2)).unwrap();
    assert_eq!(value(&managers), 600);
    assert_eq!(tick.after.act_common_params.as_deref(), Some("790#60.0"));

    managers.sync_entities(&mut fight);
    assert_eq!(value(&BattleManagers::seeded(&fight)), 600);

    let refresh = managers.buff.execute(&managers.hp, grant()).unwrap();
    let [old] = refresh.change.removed.as_slice() else {
        panic!("expected replacement of the original instance");
    };
    assert_eq!(old.buff.uid, Some(uid));
    assert_eq!(old.buff.act_common_params.as_deref(), Some("790#60.0"));
    let fresh = refresh.change.added.unwrap().buff;
    assert_ne!(fresh.uid, Some(uid));
    assert_eq!(fresh.act_common_params.as_deref(), Some("790#87.5"));
    assert_eq!(value(&managers), 875);

    managers.buff.update(10, 30, Some(120), None, None).unwrap();
    assert_eq!(value(&managers), 875);
    managers.buff.execute(&managers.hp, grant()).unwrap();
    assert_eq!(value(&managers), 2475);
}
