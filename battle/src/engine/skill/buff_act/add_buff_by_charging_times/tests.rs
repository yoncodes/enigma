use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    manager::field::{FieldCommand, FieldDefinition, FieldOperation},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn charging_buff_waits_for_a_real_team_transfer() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: [1, 2]
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    current_hp: Some(1),
                    team_type: Some(1),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 10,
        buff_id: 435411,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEndEntitySettlement,
            crate::engine::skill::rule::DefinitionKey::new(1027, "AddBuffByChargingTimes"),
        ),
        act_type: "AddBuffByChargingTimes".to_owned(),
        effect_time: 307,
        effect_condition: 0,
        args: vec![435421, 1, 0, 1],
        raw: "1027#435421#1#0#1".to_owned(),
    };

    assert!(rule_ops(&managers, &pool, &subscriber).is_empty());
    managers
        .execute_field(FieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(50019, "AddMagicCircle"),
            },
            team: 1,
            operation: FieldOperation::DeployIfAbsent {
                definition: FieldDefinition {
                    field_id: 1,
                    duration: 3,
                },
                create_uid: 1,
                initial_level: 1,
                thresholds: Vec::new(),
            },
        })
        .unwrap();
    managers
        .execute_field(FieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(50019, "AddMagicCircle"),
            },
            team: 1,
            operation: FieldOperation::ChangeProgress { delta: 1 },
        })
        .unwrap();
    let outputs = rule_ops(&managers, &pool, &subscriber);
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs
            .iter()
            .all(|output| matches!(output, RuleOp::Command(BattleCommand::Buff(_))))
    );
    assert!(!managers.buff.has_buff_id(1, 435421));
}
