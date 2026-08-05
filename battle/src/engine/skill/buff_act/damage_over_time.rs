use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{
        DamageFormulaInput, attribute_scaled_damage, calculate_with_trace, handler as damage,
    },
    entity::attr::AttrId,
    event::kind::EventKind,
    manager::{
        BattleManagers,
        buff::{BuffAmount, BuffCommand, BuffRemove, BuffRemoveSelector, BuffSetAmount},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        buff_act::{
            self,
            registry::{self, BuffActKind},
        },
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

#[derive(Clone, Copy)]
enum DamageOverTimeKind {
    Burn,
    Dot,
    Poison,
    Radiance,
}

impl DamageOverTimeKind {
    fn from_subscriber(subscriber: &BuffActSubscriber) -> Option<Self> {
        match registry::kind(subscriber.key.definition.opcode, &subscriber.act_type)? {
            BuffActKind::Burn => Some(Self::Burn),
            BuffActKind::Dot => Some(Self::Dot),
            BuffActKind::Poison => Some(Self::Poison),
            BuffActKind::Radiance => Some(Self::Radiance),
            _ => None,
        }
    }
}

pub fn supports_dot(args: &[i32]) -> bool {
    matches!(
        args,
        [0, attr_id, permille] if *attr_id == AttrId::Attack.id() && *permille > 0
    ) || matches!(
        args,
        [1, attr_id, permille]
            if matches!(
                AttrId::from_raw(*attr_id),
                Some(AttrId::CurrentHp | AttrId::Hp)
            ) && *permille > 0
    )
}

pub fn matches(subscriber: &BuffActSubscriber) -> bool {
    DamageOverTimeKind::from_subscriber(subscriber).is_some()
}

pub fn damage_rule_op(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
) -> Option<RuleOp> {
    damage_rule_op_with_duration(managers, pool, determinism, subscriber, false)
}

pub fn damage_rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
) -> Vec<RuleOp> {
    let Some(damage) = damage_rule_op(managers, pool, determinism, subscriber) else {
        return Vec::new();
    };
    let mut ops = Vec::new();
    if matches!(
        DamageOverTimeKind::from_subscriber(subscriber),
        Some(DamageOverTimeKind::Burn | DamageOverTimeKind::Dot | DamageOverTimeKind::Radiance)
    ) && let Some(effect_type) =
        super::wire::find(subscriber.key.definition.opcode, &subscriber.act_type).and_then(|wire| {
            wire.markers(super::wire::WirePhase::Static)
                .first()
                .or_else(|| wire.markers(super::wire::WirePhase::Add).first())
        })
    {
        ops.push(RuleOp::BuffFeatureMarker {
            target_uid: subscriber.owner_uid,
            effect_type: *effect_type,
            effect_num: subscriber.buff_id,
            buff_act_id: subscriber.key.definition.opcode,
        });
    }
    ops.push(damage);
    ops
}

pub fn layered_damage_rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
) -> Vec<RuleOp> {
    let mut layer = subscriber.clone();
    layer.amount = 1;
    (0..subscriber.amount.max(1))
        .flat_map(|_| damage_rule_ops(managers, pool, determinism, &layer))
        .collect()
}

pub(crate) fn detonation_rule_op(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
    consume: bool,
) -> Option<RuleOp> {
    damage_rule_op_with_duration(managers, pool, determinism, subscriber, consume)
}

fn damage_rule_op_with_duration(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
    remaining_duration: bool,
) -> Option<RuleOp> {
    let kind = DamageOverTimeKind::from_subscriber(subscriber)?;
    let origin = super::command_origin(subscriber)?;
    let source_uid = if subscriber.source_uid != 0 {
        subscriber.source_uid
    } else {
        subscriber.owner_uid
    };
    let mut amount = match kind {
        DamageOverTimeKind::Poison if remaining_duration => {
            managers.buff.dot_remaining_damage_for_owner(
                subscriber.owner_uid,
                subscriber.key.definition.opcode,
                subscriber.buff_uid,
            )?
        }
        DamageOverTimeKind::Dot if subscriber.args.first() == Some(&0) => {
            managers.buff.dot_damage_for_owner(
                subscriber.owner_uid,
                subscriber.key.definition.opcode,
                subscriber.buff_uid,
            )?
        }
        DamageOverTimeKind::Poison => managers.buff.dot_damage_for_owner(
            subscriber.owner_uid,
            subscriber.key.definition.opcode,
            subscriber.buff_uid,
        )?,
        DamageOverTimeKind::Burn | DamageOverTimeKind::Dot | DamageOverTimeKind::Radiance => {
            let [mode, attr_id, permille] = subscriber.args.as_slice() else {
                return None;
            };
            let attr_id = AttrId::from_raw(*attr_id)?;
            let attribute_uid = match kind {
                DamageOverTimeKind::Burn => subscriber.owner_uid,
                DamageOverTimeKind::Dot if *mode == 1 => subscriber.owner_uid,
                DamageOverTimeKind::Dot => source_uid,
                DamageOverTimeKind::Poison | DamageOverTimeKind::Radiance => source_uid,
            };
            attribute_scaled_damage(
                effective_attribute(managers, attribute_uid, attr_id),
                *permille,
                subscriber.amount.max(1),
            )
        }
    };
    let is_crit = matches!(kind, DamageOverTimeKind::Poison)
        && poison_can_crit(managers, pool, source_uid)
        && determinism.roll_hidden_crit(
            subscriber.buff_id,
            source_uid,
            subscriber.owner_uid,
            damage::crit_chance(source_uid, subscriber.owner_uid, pool, managers),
        );
    let crit_multiplier = if is_crit {
        damage::crit_damage_multiplier(source_uid, subscriber.owner_uid, pool, managers)
    } else {
        1000
    };
    let mut formula = DamageFormulaInput::genesis(
        amount,
        1_000,
        crate::engine::damage::modifiers::periodic_genesis_multiplier(
            managers,
            source_uid,
            subscriber.owner_uid,
        ),
    );
    if matches!(kind, DamageOverTimeKind::Poison) {
        formula.poison_multiplier = Some(poison_damage_multiplier(
            managers,
            source_uid,
            subscriber.owner_uid,
        ));
    }
    formula.crit_multiplier = crit_multiplier;
    formula.is_crit = is_crit;
    let trace = calculate_with_trace(formula);
    amount = trace.amount;
    if matches!(kind, DamageOverTimeKind::Burn) {
        amount = super::burn_real_hurt_fix::adjusted_amount(managers, subscriber.owner_uid, amount);
    }
    if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage) {
        eprintln!(
            "genesis buff_act={} buff={} buff_uid={} source={source_uid} target={} trace={trace:?}",
            subscriber.key.definition.opcode,
            subscriber.buff_id,
            subscriber.buff_uid,
            subscriber.owner_uid,
        );
    }
    (amount > 0).then_some(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
        HpLoss {
            origin,
            source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: source_uid,
                is_crit,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: if is_crit {
                    EffectType::Origincrit as i32
                } else {
                    EffectType::Origindamage as i32
                },
                display_amount: Some(amount),
            }),
        },
    ))))
}

fn poison_damage_multiplier(managers: &BattleManagers, source_uid: i64, target_uid: i64) -> i32 {
    let value = |uid, attr_id| {
        managers.attribute.get(uid, attr_id)
            + managers.buff.attribute_delta(uid, attr_id)
            + super::attr_from_entity::active_delta(managers, uid, attr_id)
    };
    (1_000 + value(source_uid, AttrId::PoisonDmgBonus)
        - value(target_uid, AttrId::PoisonDmgTakenReduction))
    .max(0)
}

fn poison_can_crit(managers: &BattleManagers, pool: &TargetPool, source_uid: i64) -> bool {
    let source_is_attacker = pool.source_is_attacker(source_uid);
    managers
        .buff
        .active_features(&managers.hp)
        .iter()
        .any(|feature| {
            pool.source_is_attacker(feature.owner_uid) == source_is_attacker
                && buff_act::is_kind(feature, BuffActKind::PoisonSettleCanCrit)
        })
}

fn effective_attribute(managers: &BattleManagers, uid: i64, attr_id: AttrId) -> i32 {
    match attr_id {
        AttrId::Attack => {
            let base = managers.attribute.base(uid, attr_id);
            let rate = 1000
                + managers.attribute.get(uid, attr_id)
                + managers.buff.attribute_delta(uid, attr_id);
            base * rate.max(0) / 1000
        }
        AttrId::Hp => managers.hp.max(uid),
        AttrId::CurrentHp => managers.hp.current(uid),
        AttrId::LostHp => (managers.hp.max(uid) - managers.hp.current(uid)).max(0),
        _ => managers.attribute.get(uid, attr_id) + managers.buff.attribute_delta(uid, attr_id),
    }
}

pub fn settlement_rule_op(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
) -> Option<RuleOp> {
    let kind = DamageOverTimeKind::from_subscriber(subscriber)?;
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = buff.layer.unwrap_or_default();
    let next = settled_layer(kind, current)?;
    if next == 0 {
        Some(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Deactivate(BuffRemove {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
            }),
        )))
    } else {
        Some(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::SetAmount(BuffSetAmount {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                amount: BuffAmount::Layer(next),
            }),
        )))
    }
}

pub fn settlement_rule_ops(
    managers: &BattleManagers,
    owner_uids: &[i64],
) -> Vec<(BuffActSubscriber, RuleOp)> {
    crate::engine::skill::subscriber::active_buffs_for_owners(
        managers,
        EventKind::RoundEnd,
        owner_uids,
    )
    .into_iter()
    .filter(matches)
    .filter_map(|subscriber| settlement_rule_op(managers, &subscriber).map(|op| (subscriber, op)))
    .collect()
}

fn settled_layer(kind: DamageOverTimeKind, current: i32) -> Option<i32> {
    match kind {
        DamageOverTimeKind::Burn | DamageOverTimeKind::Radiance => Some(current / 2),
        DamageOverTimeKind::Dot => None,
        DamageOverTimeKind::Poison => None,
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::{bus::EventBus, kind::EventKind, subscription::SubscriptionKey},
        runtime::executor::execute_rule_op,
    };

    #[test]
    fn burn_formula_uses_owner_attack_rate_per_stack() {
        assert_eq!(1441 * 40 * 8 / 1000, 461);
        assert_eq!(1441 * 40 * 3 / 1000, 172);
    }

    #[test]
    fn radiance_halves_its_stacks() {
        assert_eq!(settled_layer(DamageOverTimeKind::Radiance, 4), Some(2));
        assert_eq!(settled_layer(DamageOverTimeKind::Radiance, 1), Some(0));
        assert_eq!(settled_layer(DamageOverTimeKind::Burn, 4), Some(2));
        assert_eq!(settled_layer(DamageOverTimeKind::Burn, 1), Some(0));
    }

    #[test]
    fn periodic_genesis_damage_emits_its_registered_marker_with_the_transaction() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    attr: Some(sonettobuf::HeroAttribute {
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31340001,
            team_type: 2,
            owner_alive: true,
            amount: 2,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                crate::engine::skill::rule::DefinitionKey::new(1048, "Radiance"),
            ),
            act_type: "Radiance".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![0, 102, 40],
            raw: "1048#0#102#40".to_owned(),
        };

        assert_eq!(registry::runtime_marker(subscriber.key.definition), None);
        assert!(matches!(
            damage_rule_ops(
                &managers,
                &pool,
                &mut RoundDeterminism::default(),
                &subscriber,
            )
            .as_slice(),
            [
                RuleOp::BuffFeatureMarker {
                    target_uid: -1,
                    effect_num: 31340001,
                    buff_act_id: 1048,
                    ..
                },
                RuleOp::Command(BattleCommand::Hp(_))
            ]
        ));

        subscriber.key = SubscriptionKey::new(
            EventKind::RoundEnd,
            crate::engine::skill::rule::DefinitionKey::new(726, "Burn"),
        );
        subscriber.act_type = "Burn".to_owned();
        assert_eq!(registry::runtime_marker(subscriber.key.definition), None);

        subscriber.key = SubscriptionKey::new(
            EventKind::SkillAction,
            crate::engine::skill::rule::DefinitionKey::new(203, "Dot"),
        );
        subscriber.act_type = "Dot".to_owned();
        subscriber.effect_time = 210;
        subscriber.args = vec![1, AttrId::Hp.id(), 200];
        assert!(matches!(
            damage_rule_ops(
                &managers,
                &pool,
                &mut RoundDeterminism::default(),
                &subscriber,
            )
            .as_slice(),
            [
                RuleOp::BuffFeatureMarker {
                    effect_type,
                    buff_act_id: 203,
                    ..
                },
                RuleOp::Command(BattleCommand::Hp(_))
            ] if *effect_type == EffectType::Dot as i32
        ));
    }

    #[test]
    fn layered_dot_emits_one_current_hp_settlement_per_stack() {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(8_833),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(8_833),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 1_049,
            buff_id: 6_280_511,
            team_type: 2,
            owner_alive: true,
            amount: 3,
            key: SubscriptionKey::new(
                EventKind::RoundStart,
                crate::engine::skill::rule::DefinitionKey::new(213, "Dot"),
            ),
            act_type: "Dot".to_owned(),
            effect_time: 101,
            effect_condition: 0,
            args: vec![1, AttrId::CurrentHp.id(), 30],
            raw: "213#1#100#30".to_owned(),
        };

        let ops = layered_damage_rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber,
        );

        assert_eq!(ops.len(), 6);
        for pair in ops.chunks_exact(2) {
            assert!(matches!(
                pair,
                [
                    RuleOp::BuffFeatureMarker {
                        effect_type,
                        buff_act_id: 213,
                        ..
                    },
                    RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                        amount: 264,
                        ..
                    })))
                ] if *effect_type == EffectType::Dot as i32
            ));
        }
    }

    #[test]
    fn settlement_emits_a_command_without_mutating_the_buff() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30620111),
                        layer: Some(4),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30620111,
            team_type: 1,
            owner_alive: true,
            amount: 4,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                crate::engine::skill::rule::DefinitionKey::new(1048, "Radiance"),
            ),
            act_type: "Radiance".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args: Vec::new(),
            raw: "1048".to_owned(),
        };

        assert!(matches!(
            settlement_rule_op(&managers, &subscriber),
            Some(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::SetAmount(BuffSetAmount {
                    amount: BuffAmount::Layer(2),
                    ..
                })
            )))
        ));
        assert_eq!(managers.buff.snapshot(10, 20).unwrap().layer, Some(4));
    }

    #[test]
    fn dot_applies_source_and_target_genesis_modifiers_without_a_hit_event() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        current_hp: Some(1_000),
                        attr: Some(sonettobuf::HeroAttribute {
                            attack: Some(1_000),
                            hp: Some(1_000),
                            ..Default::default()
                        }),
                        buffs: vec![
                            BuffInfo {
                                uid: Some(1),
                                buff_id: Some(30980152),
                                ..Default::default()
                            },
                            BuffInfo {
                                uid: Some(3),
                                buff_id: Some(31050145),
                                from_uid: Some(20),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        buffs: vec![BuffInfo {
                            uid: Some(4),
                            buff_id: Some(31050111),
                            layer: Some(22),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        attack: Some(500),
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(30980126),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30,
            team_type: 2,
            owner_alive: true,
            amount: 3,
            key: SubscriptionKey::new(
                EventKind::RoundEnd,
                crate::engine::skill::rule::DefinitionKey::new(726, "Burn"),
            ),
            act_type: "Burn".to_owned(),
            effect_time: 302,
            effect_condition: 0,
            args: vec![0, 102, 40],
            raw: "726#0#102#40".to_owned(),
        };
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let output = damage_rule_op(&managers, &pool, &mut determinism, &subscriber).unwrap();
        let mut events = EventBus::default();

        assert_eq!(managers.hp.current(-1), 1_000);
        execute_rule_op(&mut managers, &mut events, output).unwrap();

        assert_eq!(managers.hp.current(-1), 901);
        assert!(matches!(
            events.pop(),
            Some(crate::engine::event::payload::BattleEvent::HpLost { amount: 99, .. })
        ));
        assert!(events.is_empty());
    }

    #[test]
    fn caster_scaled_dot_keeps_its_grant_snapshot_and_commits_hp_loss() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        attack: Some(1_000),
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(5_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(5_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let origin = crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
        };
        let dot_uid = managers
            .execute_buff(BuffCommand::Grant(
                crate::engine::manager::buff::BuffGrant {
                    origin,
                    source_uid: 10,
                    target_uid: -1,
                    buff_id: 5051,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap()
            .change
            .added
            .unwrap()
            .buff
            .uid
            .unwrap();
        managers
            .execute_buff(BuffCommand::Grant(
                crate::engine::manager::buff::BuffGrant {
                    origin,
                    source_uid: 10,
                    target_uid: 10,
                    buff_id: 2240001,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap();
        assert_eq!(managers.origin_attribute(10, AttrId::Attack), 2_000);

        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: dot_uid,
            buff_id: 5051,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(203, "Dot"),
            ),
            act_type: "Dot".to_owned(),
            effect_time: 210,
            effect_condition: 0,
            args: vec![0, AttrId::Attack.id(), 1_000],
            raw: "203#0#102#1000".to_owned(),
        };
        let pool = TargetPool::from_fight(&fight);
        let output = damage_rule_op(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber,
        )
        .unwrap();
        let mut events = EventBus::default();

        execute_rule_op(&mut managers, &mut events, output).unwrap();

        assert_eq!(managers.hp.current(-1), 4_000);
        assert!(matches!(
            events.pop(),
            Some(crate::engine::event::payload::BattleEvent::HpLost {
                source_uid: 10,
                target_uid: -1,
                amount: 1_000,
                ..
            })
        ));
    }

    #[test]
    fn holder_scaled_dot_reads_current_hp_when_the_action_triggers() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let origin = crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
        };
        let buff_uid = managers
            .execute_buff(BuffCommand::Grant(
                crate::engine::manager::buff::BuffGrant {
                    origin,
                    source_uid: 10,
                    target_uid: 10,
                    buff_id: 117361042,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))
            .unwrap()
            .change
            .added
            .unwrap()
            .buff
            .uid
            .unwrap();
        managers
            .execute_hp(HpCommand::Lose(HpLoss {
                origin,
                source_uid: 10,
                target_uid: 10,
                amount: 200,
                config_effect: 0,
                hurt: None,
            }))
            .unwrap();

        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid,
            buff_id: 117361042,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(203, "Dot"),
            ),
            act_type: "Dot".to_owned(),
            effect_time: 210,
            effect_condition: 0,
            args: vec![1, AttrId::CurrentHp.id(), 100],
            raw: "203#1#100#100".to_owned(),
        };
        let output = damage_rule_op(
            &managers,
            &TargetPool::from_fight(&fight),
            &mut RoundDeterminism::default(),
            &subscriber,
        )
        .unwrap();

        execute_rule_op(&mut managers, &mut EventBus::default(), output).unwrap();

        assert_eq!(managers.hp.current(10), 720);
    }
}
