use super::*;

#[test]
fn round_start_keeps_the_conditional_ally_riposte_before_its_sibling() {
    use crate::engine::skill::rule::route::{ConditionDriver, ConditionSetup};

    init_config();
    let db = config::configs::get();
    let catalog = SkillEffectCatalog::from_roots(db, [312101245], []);
    let slot = &catalog.get(312101245).unwrap().slots[0];
    let unrelated = ParsedBehavior::new(1210001, "AddBuff", vec![312101245]);
    let route = ConditionRoute::compile_configured(db, &slot.conditions, &unrelated).unwrap();
    assert!(matches!(
        route.branches[0].driver,
        Some(ConditionDriver::Setup(ConditionSetup {
            stage: SetupStage::BuffGate,
            priority: 0,
            ..
        }))
    ));
    for (buffs, expected) in [
        (Vec::new(), vec![312101245, 312101246]),
        (
            vec![BuffInfo {
                uid: Some(100),
                from_uid: Some(10),
                buff_id: Some(31210132),
                duration: Some(2),
                ..Default::default()
            }],
            vec![312101246],
        ),
    ] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    position: Some(1),
                    current_hp: Some(100),
                    passive_skill: vec![312101245],
                    buffs,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let setup = crate::engine::runtime::drain::run_setup_stage(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            SetupStage::RoundStart,
            1,
        )
        .unwrap();
        let grants = setup
            .events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::BuffAdded(change) => Some(change.buff_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(grants, expected);
    }
}
