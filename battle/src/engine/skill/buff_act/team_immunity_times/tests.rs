use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn round_start_resets_the_configured_team_control_immunity_budget() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    buff_id: Some(31430144),
                    uid: Some(20),
                    from_uid: Some(10),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1126),
                        param: vec![1],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| super::super::is_kind(feature, BuffActKind::TeamImmunityTimes))
        .unwrap();

    let ops = setup_rule_ops(&managers, &feature, SetupStage::RoundStart).unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
                BuffSetState {
                    act_info,
                    ..
                }
            ))),
            RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
                act_id: 1126,
                params,
                ..
            })
        ] if act_info.as_ref().unwrap()[0].param == [4] && params == &[4]
    ));
}
