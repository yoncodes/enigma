use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        toughness::{STANDARD_DAMAGE_RATE_PERMILLE, ToughnessRecord},
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
}
