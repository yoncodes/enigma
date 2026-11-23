use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use sonettobuf::{CardInfo, Fight, FightTeam};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(40006, "MonsterChange"),
};

fn managers_with_ai_action() -> BattleManagers {
    crate::test_support::init_config();
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
                origin: ORIGIN,
                cards: vec![CardInfo {
                    uid: Some(-7),
                    skill_id: Some(251401),
                    ..Default::default()
                }],
            },
        ))
        .unwrap();
    managers
}

fn command(reset_state: i32) -> MonsterChangeCommand {
    MonsterChangeCommand {
        entity: EntityCommand {
            origin: ORIGIN,
            source_uid: -7,
            target_uid: -7,
            operation: EntityOperation::Transform {
                model_id: 251407,
                parameters: [1000, reset_state],
            },
        },
    }
}

#[test]
fn reset_mode_invalidates_stale_ai_actions_and_resets_toughness() {
    let mut managers = managers_with_ai_action();

    let changes = execute(&mut managers, command(1)).unwrap();

    assert_eq!(changes.entity.entity.model_id, Some(251407));
    assert_eq!(changes.toughness.unwrap().target_uid, -7);
    let cards = changes.cards.unwrap();
    assert_eq!(cards.ai_invalidations.len(), 1);
    assert_eq!(cards.ai_invalidations[0].card_index, 1);
    assert_eq!(cards.ai_invalidations[0].team_type, 2);
    assert_eq!(cards.ai_invalidations[0].config_effect, 40006);
    assert!(managers.card.ai_queue().is_empty());
}

#[test]
fn preserve_mode_keeps_ai_actions_without_reset_markers() {
    let mut managers = managers_with_ai_action();

    let changes = execute(&mut managers, command(0)).unwrap();

    assert!(changes.toughness.is_none());
    assert!(changes.cards.is_none());
    assert_eq!(managers.card.ai_queue().len(), 1);
}
