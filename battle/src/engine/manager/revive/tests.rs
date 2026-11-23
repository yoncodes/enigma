use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

fn command() -> ReviveCommand {
    ReviveCommand {
        origin: CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(1043, "Revive"),
        },
        source_uid: 10,
        target_uid: 10,
        amount: 300,
        buff_uid: 20,
        dispel_statuses: Vec::new(),
    }
}

#[test]
fn only_the_successful_revive_consumes_its_exact_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(0),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31250181),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);

    let first = execute(&mut managers, command()).unwrap();
    let second = execute(&mut managers, command()).unwrap();

    assert_eq!(managers.hp.current(10), 300);
    assert!(first.hp.is_some());
    assert_eq!(first.buffs.len(), 1);
    assert_eq!(second, ReviveChanges::default());
}
