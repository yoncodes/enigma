use super::*;
use crate::engine::{event::kind::EventKind, skill::subscriber};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn appends_exact_color_codes_to_the_buff_owned_state() {
    assert_eq!(append(None, 897, 1, 1).as_deref(), Some("897#1"));
    assert_eq!(
        append(Some("897#1#1"), 897, 3, 1).as_deref(),
        Some("897#1#1#3")
    );
    assert!(append(Some("896#1"), 897, 2, 1).is_none());
}

#[test]
fn captured_sequence_selects_peace_and_preserves_live_totals() {
    let colors = [1, 1, 3, 2, 1, 3];
    assert_eq!(totals(&colors), (4, 8));
    assert_eq!(decimal_half(4), "2.0");
    assert_eq!(decimal_half(8), "4.0");
}

#[test]
fn captured_threshold_clears_state_marks_totals_and_casts_peace_skill() {
    crate::test_support::init_config();
    let wire = super::super::wire::find(897, "RedOrBlueCount")
        .expect("captured counter must own its wire markers");
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[EffectType::Redorbluecount as i32]
    );
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Refresh),
        &[EffectType::Redorbluecountchange as i32]
    );
    assert_eq!(
        wire.snapshot_reserve_str(Some("897#1#1#3")),
        Some("1#1#3".to_owned())
    );
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                team_type: Some(1),
                current_hp: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(1195),
                    buff_id: Some(31100551),
                    from_uid: Some(1),
                    act_common_params: Some("897#1#1#3#2#1#3".to_owned()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let subscriber = subscriber::for_active_buffs(&managers, EventKind::AllyAction)
        .into_iter()
        .find(|subscriber| subscriber.key.definition.opcode == 897)
        .expect("captured counter buff must subscribe to allied actions");

    let ops = rule_ops(
        &managers,
        &subscriber,
        &BattleEvent::AllyAction(Default::default()),
    )
    .expect("captured counter state must resolve");

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(state))),
            RuleOp::EffectMarker {
                effect_type,
                reserve_id: Some(31100563),
                reserve_str: Some(totals),
                ..
            },
            RuleOp::Skill(invocation),
        ] if state.params.as_deref() == Some("")
            && *effect_type == EffectType::Redorbluecountexskill as i32
            && totals == "2.0#4.0"
            && invocation.plan.skill_id == 31100563
    ));
}
