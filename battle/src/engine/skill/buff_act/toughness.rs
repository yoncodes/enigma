use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        toughness::{STANDARD_DAMAGE_RATE_PERMILLE, ToughnessRecord, ToughnessRecover},
    },
    skill::{
        behavior::{classify::BehaviorKind, registry as behavior_registry},
        buff_act::registry::BuffActKind,
        rule::{
            RuleDomain,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub fn recover_rule_ops(
    subscriber: &crate::engine::skill::subscriber::BuffActSubscriber,
) -> Option<Vec<RuleOp>> {
    let [config_effect] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::ToughnessRecover(
        ToughnessRecover {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            config_effect: *config_effect,
        },
    ))])
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::HpLost {
        origin,
        target_uid,
        amount,
        ..
    } = event
    else {
        return Vec::new();
    };
    if origin.domain != RuleDomain::Behavior
        || behavior_registry::find_key(origin.key.opcode, origin.key.type_name)
            .is_none_or(|definition| definition.kind != BehaviorKind::ToughnessOverflowDamage)
        || !managers.toughness.is_broken(*target_uid)
    {
        return Vec::new();
    }

    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == *target_uid
                && super::is_kind(feature, BuffActKind::ToughnessOverflowRecord)
        })
        .min_by_key(|feature| feature.buff_uid)
        .map(|feature| {
            (
                feature,
                RuleOp::Command(BattleCommand::ToughnessRecord(ToughnessRecord {
                    target_uid: *target_uid,
                    damage: *amount,
                    rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
                })),
            )
        })
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill::rule::{CommandOrigin, DefinitionKey};
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    fn managers() -> BattleManagers {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    toughness_value: Some(1),
                    toughness_point: Some(1),
                    is_broken: Some(false),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(116_362_200),
                        from_uid: Some(-1),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        managers.toughness.reduce(-1, 1, true);
        managers
    }

    fn hp_lost(key: DefinitionKey) -> BattleEvent {
        BattleEvent::HpLost {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key,
            },
            source_uid: -1,
            skill_id: 116_362_200,
            target_uid: -1,
            amount: 203_000,
            buff_uid: None,
        }
    }

    #[test]
    fn exact_overflow_loss_routes_to_the_toughness_manager() {
        let ops = transaction_rule_ops(
            &managers(),
            &hp_lost(DefinitionKey::new(60_310, "LostLife")),
        );

        assert!(matches!(
            ops.as_slice(),
            [(
                ActiveBuffFeature {
                    buff_id: 116_362_200,
                    ..
                },
                RuleOp::Command(BattleCommand::ToughnessRecord(ToughnessRecord {
                    target_uid: -1,
                    damage: 203_000,
                    rate_permille: STANDARD_DAMAGE_RATE_PERMILLE,
                }))
            )]
        ));
    }

    #[test]
    fn another_lost_life_opcode_does_not_share_the_route() {
        assert!(
            transaction_rule_ops(
                &managers(),
                &hp_lost(DefinitionKey::new(30_005, "LostLife"))
            )
            .is_empty()
        );
    }

    #[test]
    fn recovery_buff_act_emits_the_existing_toughness_command() {
        let subscriber = crate::engine::skill::subscriber::BuffActSubscriber {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 10,
            buff_id: 118350001,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::RoundStart,
                DefinitionKey::new(1102, "ToughnessRecover"),
            ),
            act_type: "ToughnessRecover".into(),
            effect_time: 101,
            effect_condition: 0,
            args: vec![0],
            raw: "1102#0".into(),
        };

        assert!(matches!(
            recover_rule_ops(&subscriber).as_deref(),
            Some([RuleOp::Command(BattleCommand::ToughnessRecover(
                ToughnessRecover {
                    target_uid: -1,
                    config_effect: 0,
                    ..
                }
            ))])
        ));
    }
}
