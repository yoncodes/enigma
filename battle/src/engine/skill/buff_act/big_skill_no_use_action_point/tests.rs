use super::*;
use crate::engine::skill::buff_act::{is_kind, registry::BuffActKind};
use crate::engine::{event::kind::EventKind, skill::action::ActionEvent};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn completed_owner_ultimate_consumes_the_exact_buff_instance() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(2),
                    buff_id: Some(31280116),
                    from_uid: Some(10),
                    count: Some(1),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let features = managers.buff.active_features(&managers.hp);
    assert!(
        features
            .iter()
            .any(|feature| is_kind(feature, BuffActKind::BigSkillNoUseActPoint)),
        "features={features:?}"
    );
    let subscribers =
        crate::engine::skill::subscriber::for_active_buffs(&managers, EventKind::AllyAction);
    let [subscriber] = subscribers.as_slice() else {
        panic!("expected exact 946 subscriber")
    };
    let event = BattleEvent::AllyAction(ActionEvent {
        source_uid: 10,
        target_uid: -1,
        skill_id: 31280131,
        skill_slot: 3,
        is_attack: true,
        rank: 1,
        skill_type: 0,
        effect_tag: 1,
        additional_moxie: 0,
        extra_skill_kind: 0,
        assassinate: false,
        ..Default::default()
    });

    assert!(matches!(
        rule_ops(
            &managers,
            crate::engine::skill::effect::catalog::global(),
            subscriber,
            &event,
        )
        .unwrap()
        .as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
            BuffRemove {
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(2),
                ..
            }
        )))]
    ));
}
