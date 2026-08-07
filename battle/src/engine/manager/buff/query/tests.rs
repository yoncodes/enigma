use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use crate::engine::manager::BattleManagers;

#[test]
fn unsupported_special_moxie_arguments_do_not_change_ultimate_cost() {
    crate::test_support::init_config();
    let entity = FightEntityInfo {
        uid: Some(10),
        current_hp: Some(100),
        team_type: Some(1),
        buffs: vec![sonettobuf::BuffInfo {
            uid: Some(1),
            buff_id: Some(31_000_161),
            from_uid: Some(10),
            layer: Some(1),
            count: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity],
            ..Default::default()
        }),
        ..Default::default()
    });
    let definition = managers.buff.buffs[0].definition.as_mut().unwrap();
    definition.replace_features_for_test(super::super::feature::resolve_features("832#7#-3"));

    assert_eq!(
        managers.buff.buff_act_argument_scalar(
            10,
            crate::engine::skill::buff_act::registry::BuffActKind::SpExPointMaxAdd,
            1,
        ),
        0
    );
}

#[test]
fn ulrich_channels_project_configured_enemy_and_ally_outputs() {
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
    managers.buff.add(&managers.hp, 10, 10, 31070121, 0);
    managers.buff.add_special_count(10, &[31070111], 3);
    managers.buff.add_special_count(10, &[31070121], 3);

    let outputs = managers.buff.special_count_outputs(&managers.hp);

    assert_eq!(
        outputs
            .iter()
            .map(|output| (output.target_uid, output.output_buff_id, output.amount))
            .collect::<Vec<_>>(),
        vec![
            (-1, 31070141, -420),
            (-2, 31070141, -420),
            (10, 31070151, 420),
            (11, 31070151, 420),
        ]
    );
}
