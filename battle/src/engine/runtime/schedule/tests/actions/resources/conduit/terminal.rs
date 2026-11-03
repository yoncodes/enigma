use super::*;

#[test]
fn conduit_admission_observes_committed_terminal_outcomes() {
    init_config();
    for (wave, terminal) in [(4, true), (1, false)] {
        let entity = |uid, model_id, hp| FightEntityInfo {
            uid: Some(uid),
            model_id: Some(model_id),
            current_hp: Some(hp),
            attr: Some(HeroAttribute {
                hp: Some(hp),
                attack: Some(10_000),
                ..Default::default()
            }),
            ..Default::default()
        };
        let fight = Fight {
            battle_id: Some(2514),
            version: Some(7),
            cur_wave: Some(wave),
            cur_round: Some(1),
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 3149, 100_000), entity(12, 3149, 100_000)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1, 1001, 1)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::ChangePower(
                    crate::engine::manager::conduit::ConduitPowerChange {
                        origin: CommandOrigin {
                            domain: RuleDomain::Behavior,
                            key: DefinitionKey::new(60291, "AddDevicePower"),
                        },
                        source_uid: 10,
                        team: 1,
                        power_id: 1,
                        delta: 6,
                        kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                    },
                ),
            )
            .unwrap();
        let mut catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let result = run_conduit_phase(
            managers.catalog(),
            &fight,
            &mut managers,
            &pool,
            &mut catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                battle_id: 2514,
                current_round: 1,
                ..Default::default()
            },
            &[10, 12].map(|uid| sonettobuf::FightDeviceOper {
                uid: Some(uid),
                index: Some(1),
            }),
        )
        .unwrap();

        let victory = crate::engine::round::outcome::BattleOutcome::Victory;
        assert_eq!(managers.hp.current(-1), 0);
        assert_eq!(managers.terminal_outcome(), terminal.then_some(victory));
        assert_eq!(
            result
                .events
                .iter()
                .filter(|event| matches!(event,
                    BattleEvent::BattleTerminalCommitted { outcome, .. } if *outcome == victory
                ))
                .count(),
            usize::from(terminal)
        );
        for finished in [false, true] {
            let occurred = |source_uid, expected_skill| {
                result.outcomes.iter().any(|outcome| match outcome {
                    RuleOutcome::Conduit(
                        crate::engine::manager::conduit::ConduitChange::SkillBegan {
                            source_uid: owner,
                            skill_id,
                            ..
                        },
                    ) if !finished => *owner == source_uid && *skill_id == expected_skill,
                    RuleOutcome::Conduit(
                        crate::engine::manager::conduit::ConduitChange::SkillFinished {
                            source_uid: owner,
                            skill_id,
                            ..
                        },
                    ) if finished => *owner == source_uid && *skill_id == expected_skill,
                    _ => false,
                })
            };
            assert!(occurred(10, 31490121));
            assert_eq!(occurred(12, 31490111), !terminal);
        }
        assert_eq!(managers.conduit.uses(10), 2);
        assert_eq!(managers.conduit.uses(12), i32::from(!terminal));
        assert_eq!(managers.ex_point.get(12), if terminal { 0 } else { 8 });
        assert_eq!(managers.conduit.power(1, 1), if terminal { 7 } else { 11 });
    }
}
