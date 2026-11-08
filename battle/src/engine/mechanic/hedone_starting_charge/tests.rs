use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        card::{CardAddHedone, CardCommand, CardSetup},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::*;

#[test]
fn overflow_fails_before_card_or_carrier_mutation() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-4),
                model_id: Some(3156),
                team_type: Some(1),
                current_hp: Some(10_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60333, "HedoneAddCard"),
    };
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: -4,
            target_uid: -4,
            buff_id: 31560185,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    managers
        .execute_card(CardCommand::AddHedone(CardAddHedone {
            origin,
            owner_uid: -4,
            skill_id: 31560181,
            hero_id: 3156,
            team_type: 1,
        }))
        .unwrap();
    let before_card = managers.card.hand().to_vec();
    let before_buff = managers
        .buff
        .active_for(-4)
        .find(|buff| buff.buff_id == Some(31560185))
        .cloned()
        .unwrap();

    assert_eq!(
        execute(
            &mut managers,
            HedoneStartingChargeCommand {
                origin,
                owner_uid: -4,
                amount: 11,
            },
        ),
        Err(HedoneStartingChargeError::InvalidCommand)
    );
    assert_eq!(managers.card.hand(), before_card);
    assert_eq!(
        managers
            .buff
            .active_for(-4)
            .find(|buff| buff.buff_id == Some(31560185))
            .cloned()
            .unwrap(),
        before_buff
    );
}
