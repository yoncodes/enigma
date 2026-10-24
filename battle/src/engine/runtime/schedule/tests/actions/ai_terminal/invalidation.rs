use super::*;

fn scheduled_enemy_turn() -> AiTurnPlan {
    AiTurnPlan::new(
        Some(vec![
            AiSkillChoice {
                source_uid: -7,
                skill_id: 251401,
                target_uid: 10,
            },
            AiSkillChoice {
                source_uid: -7,
                skill_id: 251402,
                target_uid: 10,
            },
            AiSkillChoice {
                source_uid: -8,
                skill_id: 251403,
                target_uid: 10,
            },
        ]),
        &[],
    )
}

fn managers_with_scheduled_enemy_card() -> BattleManagers {
    init_config();
    let entity =
        crate::engine::fight::defender::Defender::build_monster_with_uid(251417, -7, 1, 2).unwrap();
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::SetAiQueue(
            crate::engine::manager::card::CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "AiTurnPlanTest"),
                },
                cards: vec![CardInfo {
                    uid: Some(-7),
                    skill_id: Some(251401),
                    target_uid: Some(10),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    managers
}

#[test]
fn card_invalidation_cancels_only_the_matching_scheduled_enemy() {
    let mut managers = managers_with_scheduled_enemy_card();
    let changes = managers
        .execute_card(CardCommand::InvalidateAiOwner(
            crate::engine::manager::card::CardInvalidateAiOwner {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "AiTurnPlanTest"),
                },
                owner_uid: -7,
                team_type: 2,
            },
        ))
        .unwrap();
    let mut plan = scheduled_enemy_turn();
    plan.observe(&DrainResult {
        outcomes: vec![RuleOutcome::Card(Box::new(changes))],
        ..Default::default()
    });

    assert_eq!(
        plan.into_choices(),
        vec![AiSkillChoice {
            source_uid: -8,
            skill_id: 251403,
            target_uid: 10,
        }]
    );
}

#[test]
fn monster_change_invalidation_cancels_only_the_matching_scheduled_enemy() {
    let mut managers = managers_with_scheduled_enemy_card();
    let changes = crate::engine::mechanic::monster_change::execute(
        &mut managers,
        crate::engine::mechanic::monster_change::MonsterChangeCommand {
            entity: crate::engine::manager::entity::EntityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(40006, "MonsterChange"),
                },
                source_uid: -7,
                target_uid: -7,
                operation: crate::engine::manager::entity::EntityOperation::Transform {
                    model_id: 251407,
                    parameters: [1_000, 1],
                },
            },
        },
    )
    .unwrap();
    let mut plan = scheduled_enemy_turn();
    plan.observe(&DrainResult {
        outcomes: vec![RuleOutcome::MonsterChange(Box::new(changes))],
        ..Default::default()
    });

    assert_eq!(
        plan.into_choices(),
        vec![AiSkillChoice {
            source_uid: -8,
            skill_id: 251403,
            target_uid: 10,
        }]
    );
}
