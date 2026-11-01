use super::*;
use crate::engine::manager::eureka::{EurekaChange, EurekaChanges, EurekaCommand};

#[test]
fn heat_overload_blocks_only_the_configured_power_gain() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            assist_boss: Some(FightEntityInfo {
                uid: Some(-1), team_type: Some(1), current_hp: Some(1000),
                attr: Some(HeroAttribute { hp: Some(1000), ..Default::default() }),
                power_infos: vec![
                    PowerInfo { power_id: Some(4), num: Some(10), max: Some(100) },
                    PowerInfo { power_id: Some(1), num: Some(0), max: Some(5) },
                ],
                buffs: vec![BuffInfo { uid: Some(77), buff_id: Some(12410011),
                    from_uid: Some(-1), layer: Some(1), ..Default::default() }],
                ..Default::default()
            }), ..Default::default()
        }), ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin { domain: RuleDomain::Behavior, key: DefinitionKey::new(50017, "ChangePower") };
    let change = |power_id, delta| EurekaCommand::Change(EurekaChange {
        origin, source_uid: -1, target_uid: -1, power_id, delta,
        effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
    });
    assert!(matches!(managers.execute_eureka(change(4, 20)).unwrap(), EurekaChanges::Unchanged { .. }));
    assert_eq!(managers.eureka.get(-1, 4).current, 10);
    managers.execute_eureka(change(4, -3)).unwrap();
    assert_eq!(managers.eureka.get(-1, 4).current, 7);
    managers.execute_eureka(change(1, 2)).unwrap();
    assert_eq!(managers.eureka.get(-1, 1).current, 2);
    managers.execute_buff(BuffCommand::Remove(BuffRemove {
        origin, target_uid: -1, selector: BuffRemoveSelector::ExactId(12410011),
    })).unwrap();
    managers.execute_eureka(change(4, 20)).unwrap();
    assert_eq!(managers.eureka.get(-1, 4).current, 27);
}
