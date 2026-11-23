use super::*;

#[test]
fn burn_returns_count_admitted_layers_before_the_cap() {
    crate::test_support::init_config();
    const BURN: i32 = 4150001;
    let buff = |uid, buff_id, layer| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(10),
        layer,
        ..Default::default()
    };
    for (resident, expected_attempts) in [
        (buff(21, BURN, Some(10)), 24),
        (buff(21, BURN, Some(30)), 24),
        (
            BuffInfo {
                uid: None,
                duration: Some(0),
                ..buff(21, BURN, Some(30))
            },
            24,
        ),
        (buff(21, 11730031, None), 0),
    ] {
        let restored_at_cap = resident.uid.is_none();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    passive_skill: vec![200],
                    buffs: vec![buff(20, BURN, Some(24))],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    buffs: vec![resident],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let mut passive = SkillEffectSlot::new(
            ParsedBehavior::new(60059, "AddBurnBySkillAddBurnCount", vec![BURN, 1]),
            TargetRequest::self_only(),
        );
        passive.conditions =
            crate::engine::skill::condition::parse_conditions(config::configs::get(), "210");
        passive.compiled_route = ConditionRoute::compile(&passive.conditions);
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert(ParsedSkillEffect {
            skill_id: 100,
            slots: vec![SkillEffectSlot::new(
                ParsedBehavior::new(60068, "AddBuffByBuffLayer", vec![BURN, BURN, 1]),
                TargetRequest {
                    code: 202,
                    raw: Vec::new(),
                },
            )],
        });
        catalog.insert(ParsedSkillEffect {
            skill_id: 200,
            slots: vec![passive],
        });
        let result = run(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [RuleOp::Skill(
                SkillRequest {
                    source_uid: 10,
                    skill_id: 100,
                }
                .into(),
            )],
        )
        .unwrap();
        let attempts = result.outcomes.iter().filter(|outcome| {
            matches!(outcome, RuleOutcome::Buff(changes) if changes.origin.key.opcode == 60059)
        }).count();
        assert_eq!(attempts, expected_attempts);
        if restored_at_cap {
            assert!(result.outcomes.iter().any(|outcome| {
                matches!(outcome, RuleOutcome::Buff(changes) if changes.origin.key.opcode == 60068 && changes.change.refreshed.is_empty())
            }));
        }
        if expected_attempts == 0 {
            assert!(result.events.iter().any(|event| {
                matches!(event, BattleEvent::BuffRejected(change) if change.target_uid == -1 && change.buff_id == BURN)
            }));
        } else {
            assert_eq!(managers.buff.max_id_or_type_layer(-1, BURN), 30);
        }
    }
}
