use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    event::{payload::BuffChangeEvent, subscription::SubscriptionKey},
    manager::{
        buff::CommandOrigin,
        contract::{ContractCommand, ContractManager},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(836, "ContractCastChannel"),
    }
}

fn managers(with_lock: bool) -> BattleManagers {
    crate::test_support::init_config();
    let mut managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(1),
                    current_hp: Some(2_000),
                    attr: Some(HeroAttribute {
                        hp: Some(2_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(22),
                    team_type: Some(1),
                    ex_skill: Some(307_001_333),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: with_lock
                        .then(|| BuffInfo {
                            uid: Some(77),
                            buff_id: Some(31_000_151),
                            from_uid: Some(-1),
                            duration: Some(1),
                            ..Default::default()
                        })
                        .into_iter()
                        .collect(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    });
    managers.contract = ContractManager::default();
    managers
        .contract
        .execute(ContractCommand::Offer {
            origin: origin(),
            owner_uid: -1,
            candidates: vec![22],
        })
        .unwrap();
    managers
        .contract
        .execute(ContractCommand::SelectOwner {
            owner_uid: -1,
            bound_uid: 22,
        })
        .unwrap();
    managers
        .contract
        .execute(ContractCommand::SelectBound {
            owner_uid: -1,
            bound_uid: 22,
        })
        .unwrap();
    managers
}

#[test]
fn carrier_grant_locks_the_selected_bound_ally() {
    let ops = grant_transaction_rule_ops(
        &managers(false),
        &BattleEvent::BuffAdded(BuffChangeEvent {
            source_uid: -1,
            target_uid: -1,
            buff_uid: 1364,
            buff_id: 31_000_431,
            before_amount: 0,
            after_amount: 1,
            added_type_total: None,
            act_id: 0,
            act_value: 0,
            replaced_fixed_max_hp: None,
            replaced_buff_id: None,
            expiry: crate::engine::event::payload::BuffExpiry::None,
        }),
    );

    assert!(matches!(
        ops.as_slice(),
        [(
            _,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                source_uid: -1,
                target_uid: 22,
                buff_id: 31_000_151,
                ..
            })))
        )]
    ));
}

#[test]
fn carrier_grant_rejects_unsupported_arguments_before_mutation() {
    let feature = |values: Vec<i32>| ActiveBuffFeature {
        owner_uid: -1,
        source_uid: -1,
        buff_uid: 1364,
        buff_id: 31_000_431,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "ContractCastChannel".to_owned(),
        effect_time: 104,
        effect_condition: 0,
        raw: values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join("#"),
        values,
    };
    let managers = managers(false);

    assert!(
        grant_rule_op(
            &managers,
            &feature(vec![836, 1, -150, 31_000_151, 31_000_441])
        )
        .is_none()
    );
    assert!(grant_rule_op(&managers, &feature(vec![836, 1, 150, 0, 31_000_441])).is_none());
    assert!(grant_rule_op(&managers, &feature(vec![836, 1, 150, 31_000_151, 0])).is_none());
}

#[test]
fn round_start_loses_hp_casts_both_ultimates_then_removes_the_lock() {
    let managers = managers(true);
    let subscriber = BuffActSubscriber {
        owner_uid: -1,
        source_uid: -1,
        buff_uid: 88,
        buff_id: 31_000_431,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStart,
            DefinitionKey::new(836, "ContractCastChannel"),
        ),
        act_type: "ContractCastChannel".to_owned(),
        effect_time: 104,
        effect_condition: 0,
        args: vec![1, 150, 31_000_151, 31_000_441],
        raw: "836#1#150#31000151#31000441".to_owned(),
    };

    let ops = rule_ops(
        &managers,
        &SkillEffectCatalog::from_game_db(config::configs::get()),
        &subscriber,
        &BattleEvent::RoundStart,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                source_uid: -1,
                target_uid: 22,
                amount: 150,
                hurt: Some(HurtInfoData {
                    damage_from: HurtDamageFromType::Buff,
                    buff_act_id: 836,
                    buff_uid: 88,
                    ..
                }),
                ..
            }))),
            RuleOp::Skill(SkillInvocation {
                plan: SkillRequest {
                    source_uid: 22,
                    skill_id: 307_001_333,
                },
                mode: SkillExecutionMode::Active,
                ..
            }),
            RuleOp::Skill(SkillInvocation {
                plan: SkillRequest {
                    source_uid: -1,
                    skill_id: 31_000_441,
                },
                mode: SkillExecutionMode::Active,
                ..
            }),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 22,
                selector: BuffRemoveSelector::Uid(77),
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: -1,
                selector: BuffRemoveSelector::Uid(88),
                ..
            })))
        ]
    ));
}
