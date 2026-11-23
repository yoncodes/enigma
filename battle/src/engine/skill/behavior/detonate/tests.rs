use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::skill::{
    behavior::classify::BehaviorSpec,
    target::{TargetContext, TargetPool},
};
use crate::engine::{manager::BattleManagers, runtime::determinism::RoundDeterminism};

#[test]
fn detonate_resolves_selected_cure_then_consumes_its_instance() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(600101),
                    from_uid: Some(10),
                    duration: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(20009, "Detonate2"),
        vec![5003, 0, 1, 1000],
        vec!["5003".into(), "0".into(), "1".into(), "1000".into()],
    );
    let pool = TargetPool::default();
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = Default::default();
    let mut target = TargetContext::default();

    let ops = Handler::emit_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                selector: BuffRemoveSelector::Uid(20),
                ..
            })))
        ] if heal.amount == 1_000 && heal.config_effect == 20009
    ));
}
