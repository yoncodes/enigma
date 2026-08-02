use crate::engine::{
    damage::handler as damage,
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{BattleManagers, hp::HurtDamageFromType},
    mechanic::shell::{ShellChangeKind, ShellCommand},
    runtime::determinism::RoundDeterminism,
    skill::{
        buff_act::registry::{self, BuffActKind},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellProcessSpec {
    pub stock_buff_id: i32,
    pub deployed_buff_id: i32,
    pub moxie_chance: i32,
    pub moxie_delta: i32,
    pub heal_attr_id: i32,
    pub heal_rate: i32,
}

fn process_spec_from_args(args: &[i32]) -> Option<ShellProcessSpec> {
    let [
        stock_buff_id,
        deployed_buff_id,
        moxie_chance,
        moxie_delta,
        heal_attr_id,
        heal_rate,
    ] = args
    else {
        return None;
    };
    Some(ShellProcessSpec {
        stock_buff_id: *stock_buff_id,
        deployed_buff_id: *deployed_buff_id,
        moxie_chance: *moxie_chance,
        moxie_delta: *moxie_delta,
        heal_attr_id: *heal_attr_id,
        heal_rate: *heal_rate,
    })
}

pub fn process_spec(buff_id: i32) -> Option<ShellProcessSpec> {
    let db = config::try_get()?;
    let buff = db.skill_buff.get(buff_id)?;
    buff.features.split('|').find_map(|raw| {
        let fields = raw
            .split('#')
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let [act_id, args @ ..] = fields.as_slice() else {
            return None;
        };
        let act_type = &db.buff_act.get(*act_id)?.r#type;
        let spec = process_spec_from_args(args)?;
        (registry::kind(*act_id, act_type) == Some(BuffActKind::ShellProcess)
            && (spec.stock_buff_id == buff_id || spec.deployed_buff_id == buff_id))
            .then_some(spec)
    })
}

pub fn deployed_buff_id(stock_buff_id: i32) -> Option<i32> {
    process_spec(stock_buff_id)
        .filter(|spec| spec.stock_buff_id == stock_buff_id)
        .map(|spec| spec.deployed_buff_id)
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let origin = super::command_origin(subscriber)?;
    let command = match super::subscriber_kind(subscriber)? {
        BuffActKind::ShellProcess => {
            return process_rule_ops(managers, pool, determinism, subscriber, event);
        }
        BuffActKind::Shell => {
            let BattleEvent::Hit(hit) = event else {
                return Some(Vec::new());
            };
            if hit.target_uid != subscriber.owner_uid
                || hit.amount <= 0
                || !matches!(
                    hit.damage_from,
                    HurtDamageFromType::Skill | HurtDamageFromType::ShareHurt
                )
            {
                return Some(Vec::new());
            }
            let spec = process_spec(subscriber.buff_id)?;
            let amount = subscriber.args.first().copied().unwrap_or(1).max(0);
            if amount == 0
                || managers
                    .buff
                    .buff_id_amount(subscriber.owner_uid, spec.stock_buff_id)
                    <= 0
            {
                return Some(Vec::new());
            }
            ShellCommand::Deploy {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: hit.source_uid,
                stock_buff_id: spec.stock_buff_id,
                amount,
            }
        }
        BuffActKind::ShellDebuff => {
            let BattleEvent::Hit(hit) = event else {
                return Some(Vec::new());
            };
            if hit.target_uid != subscriber.owner_uid
                || hit.amount <= 0
                || hit.damage_from != HurtDamageFromType::Skill
            {
                return Some(Vec::new());
            }
            let spec = process_spec(subscriber.buff_id)?;
            let amount = subscriber.args.first().copied().unwrap_or(1).max(0);
            if amount == 0 {
                return Some(Vec::new());
            }
            ShellCommand::Retrieve {
                origin,
                source_uid: subscriber.source_uid,
                target_uid: subscriber.owner_uid,
                stock_buff_id: spec.stock_buff_id,
                amount,
            }
        }
        BuffActKind::ShellLock => {
            let BattleEvent::ShellChanged(change) = event else {
                return Some(Vec::new());
            };
            if change.kind != ShellChangeKind::Retrieved
                || change.target_uid != subscriber.owner_uid
                || change.source_uid != subscriber.source_uid
                || change.amount <= 0
            {
                return Some(Vec::new());
            }
            ShellCommand::Deploy {
                origin,
                source_uid: change.source_uid,
                target_uid: change.target_uid,
                stock_buff_id: change.stock_buff_id,
                amount: change.amount,
            }
        }
        _ => return None,
    };
    Some(vec![RuleOp::Command(BattleCommand::Shell(command))])
}

fn process_rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let spec = process_spec_from_args(&subscriber.args)?;
    let BattleEvent::ShellChanged(change) = event else {
        return Some(Vec::new());
    };
    if subscriber.buff_id != spec.stock_buff_id
        || subscriber.owner_uid != change.source_uid
        || spec.stock_buff_id != change.stock_buff_id
    {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    match change.kind {
        ShellChangeKind::Deployed => {
            if !determinism.roll_permille(spec.moxie_chance) || spec.moxie_delta == 0 {
                return Some(Vec::new());
            }
            Some(vec![RuleOp::Command(BattleCommand::ExPoint(
                crate::engine::manager::ex_point::ExPointCommand::Change(
                    crate::engine::manager::ex_point::ExPointChange {
                        origin,
                        source_uid: subscriber.owner_uid,
                        target_uid: subscriber.owner_uid,
                        delta: spec.moxie_delta,
                        config_effect: 0,
                        effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
                    },
                ),
            ))])
        }
        ShellChangeKind::Retrieved => {
            if !change.settles_transaction || change.transaction_amount <= 0 {
                return Some(Vec::new());
            }
            let attr_id = AttrId::from_raw(spec.heal_attr_id)?;
            let base = managers
                .origin_attribute(subscriber.owner_uid, attr_id)
                .max(0)
                * spec.heal_rate.max(0)
                * change.transaction_amount
                / 1000;
            if base <= 0 {
                return Some(Vec::new());
            }
            let heals = pool
                .main_allies(subscriber.owner_uid)
                .iter()
                .filter(|ally| managers.hp.current(ally.uid) > 0)
                .map(|ally| {
                    let is_crit = determinism.roll_hidden_crit(
                        subscriber.buff_id,
                        subscriber.owner_uid,
                        ally.uid,
                        damage::crit_chance(subscriber.owner_uid, ally.uid, pool, managers),
                    );
                    let mut amount =
                        damage::modified_heal(base, subscriber.owner_uid, ally.uid, managers);
                    if is_crit {
                        amount = amount
                            * managers
                                .origin_attribute(subscriber.owner_uid, AttrId::CriticalDmg)
                                .max(0)
                            / 1000;
                    }
                    crate::engine::manager::hp::HpCommand::Heal(
                        crate::engine::manager::hp::HpHeal {
                            origin,
                            source_uid: subscriber.owner_uid,
                            target_uid: ally.uid,
                            amount,
                            config_effect: 0,
                            kind: if is_crit {
                                crate::engine::manager::hp::HpHealKind::Critical
                            } else {
                                crate::engine::manager::hp::HpHealKind::Normal
                            },
                        },
                    )
                })
                .collect::<Vec<_>>();
            if heals.is_empty() {
                Some(Vec::new())
            } else {
                Some(vec![RuleOp::Command(BattleCommand::HpBatch(heals))])
            }
        }
    }
}

pub fn extra_action_attribute_delta(
    feature: &crate::engine::manager::buff::ActiveBuffFeature,
    attr_id: AttrId,
) -> i32 {
    if !super::is_kind(feature, BuffActKind::ShellDebuff) {
        return 0;
    }
    feature.values[2..]
        .chunks_exact(2)
        .find_map(|pair| {
            (AttrId::from_raw(pair[0]) == Some(attr_id)).then_some(pair[1] * feature.amount)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{
            kind::EventKind,
            payload::{HitEvent, ShellChangeEvent},
            subscription::SubscriptionKey,
        },
        manager::buff::CommandOrigin,
        skill::rule::{DefinitionKey, RuleDomain},
    };

    fn subscriber(
        owner_uid: i64,
        source_uid: i64,
        buff_id: i32,
        act_id: i32,
        act_type: &'static str,
        args: Vec<i32>,
    ) -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid,
            source_uid,
            buff_uid: 20,
            buff_id,
            team_type: 1,
            owner_alive: true,
            amount: 3,
            key: SubscriptionKey::new(EventKind::BeAttacked, DefinitionKey::new(act_id, act_type)),
            act_type: act_type.to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args,
            raw: String::new(),
        }
    }

    #[test]
    fn shell_pair_comes_from_the_stock_buffs_shell_process_feature() {
        crate::test_support::init_config();

        assert_eq!(deployed_buff_id(31090111), Some(31090112));
        assert_eq!(deployed_buff_id(31090113), Some(31090114));
        assert_eq!(
            process_spec(31090118),
            Some(ShellProcessSpec {
                stock_buff_id: 31090117,
                deployed_buff_id: 31090118,
                moxie_chance: 250,
                moxie_delta: 1,
                heal_attr_id: 102,
                heal_rate: 400,
            })
        );
    }

    #[test]
    fn stock_shell_moves_one_layer_to_the_attacker_after_shared_damage() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31090111),
                        layer: Some(8),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            skill_id: 1,
            amount: 20,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::ShareHurt,
            assassinate: false,
            ignore_riposte: false,
        });

        let pool = TargetPool::from_fight(&fight);
        let ops = rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber(10, 10, 31090111, 870, "Shell", vec![1]),
            &event,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Shell(
                ShellCommand::Deploy {
                    source_uid: 10,
                    target_uid: -1,
                    stock_buff_id: 31090111,
                    amount: 1,
                    ..
                }
            ))]
        ));
    }

    #[test]
    fn shell_lock_redeploys_only_retrievals_from_its_carrier() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let event = BattleEvent::ShellChanged(ShellChangeEvent {
            kind: ShellChangeKind::Retrieved,
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090111,
            deployed_buff_id: 31090112,
            amount: 3,
            transaction_amount: 3,
            settles_transaction: true,
        });
        let ops = rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber(-1, 10, 31090131, 873, "ShellLock", Vec::new()),
            &event,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Shell(
                ShellCommand::Deploy {
                    source_uid: 10,
                    target_uid: -1,
                    stock_buff_id: 31090111,
                    amount: 3,
                    ..
                }
            ))]
        ));
    }

    #[test]
    fn deployed_shell_reads_each_configured_extra_action_attribute_per_layer() {
        let feature = crate::engine::manager::buff::ActiveBuffFeature {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31090118,
            amount: 4,
            team_type: 2,
            owner_alive: true,
            act_type: "ShellDebuff".into(),
            effect_time: 0,
            effect_condition: 0,
            raw: "871#1#203#60#205#60".into(),
            values: vec![871, 1, 203, 60, 205, 60],
        };

        assert_eq!(
            extra_action_attribute_delta(&feature, AttrId::CriticalDmg),
            240
        );
        assert_eq!(
            extra_action_attribute_delta(&feature, AttrId::DmgBonus),
            240
        );
        assert_eq!(extra_action_attribute_delta(&feature, AttrId::Attack), 0);
    }

    #[test]
    fn shell_process_uses_configured_retrieval_total_to_heal_each_living_ally_once() {
        crate::test_support::init_config();
        let entity = |uid, buff: Option<BuffInfo>| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(500),
            attr: Some(HeroAttribute {
                hp: Some(2_000),
                attack: Some(1_000),
                ..Default::default()
            }),
            buffs: buff.into_iter().collect(),
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(
                        10,
                        Some(BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31090111),
                            from_uid: Some(10),
                            layer: Some(8),
                            ..Default::default()
                        }),
                    ),
                    entity(11, None),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let event = BattleEvent::ShellChanged(ShellChangeEvent {
            kind: ShellChangeKind::Retrieved,
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090111,
            deployed_buff_id: 31090112,
            amount: 2,
            transaction_amount: 2,
            settles_transaction: true,
        });

        let ops = rule_ops(
            &managers,
            &pool,
            &mut RoundDeterminism::default(),
            &subscriber(
                10,
                10,
                31090111,
                869,
                "ShellProcess",
                vec![31090111, 31090112, 200, 1, 102, 300],
            ),
            &event,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::HpBatch(heals))]
                if matches!(
                    heals.as_slice(),
                    [
                        crate::engine::manager::hp::HpCommand::Heal(
                            crate::engine::manager::hp::HpHeal {
                                target_uid: 10,
                                amount: 600,
                                ..
                            }
                        ),
                        crate::engine::manager::hp::HpCommand::Heal(
                            crate::engine::manager::hp::HpHeal {
                                target_uid: 11,
                                amount: 600,
                                ..
                            }
                        )
                    ]
                )
        ));
    }

    #[test]
    fn shell_process_rolls_the_configured_moxie_gain_from_the_shared_rng() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let event = BattleEvent::ShellChanged(ShellChangeEvent {
            kind: ShellChangeKind::Deployed,
            source_uid: 10,
            target_uid: -1,
            stock_buff_id: 31090111,
            deployed_buff_id: 31090112,
            amount: 3,
            transaction_amount: 3,
            settles_transaction: true,
        });
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_permille_rolls([0]);

        let ops = rule_ops(
            &managers,
            &pool,
            &mut determinism,
            &subscriber(
                10,
                10,
                31090111,
                869,
                "ShellProcess",
                vec![31090111, 31090112, 200, 1, 102, 300],
            ),
            &event,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::ExPoint(
                crate::engine::manager::ex_point::ExPointCommand::Change(
                    crate::engine::manager::ex_point::ExPointChange {
                        target_uid: 10,
                        delta: 1,
                        ..
                    }
                )
            ))]
        ));
    }
}
