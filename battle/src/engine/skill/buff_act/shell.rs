use crate::{
    catalog::BattleCatalog,
    engine::{
        damage::handler as damage,
        entity::attr::AttrId,
        event::payload::BattleEvent,
        manager::{BattleManagers, hp::HurtDamageFromType},
        mechanic::shell::{ShellChangeKind, ShellCommand},
        runtime::determinism::RoundDeterminism,
        skill::{
            buff_act::registry::BuffActKind,
            rule::output::{BattleCommand, RuleOp},
            subscriber::BuffActSubscriber,
            target::TargetPool,
        },
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
    resolve_process_spec(BattleCatalog::try_global()?, buff_id)
}

pub(crate) fn resolve_process_spec(
    catalog: BattleCatalog,
    buff_id: i32,
) -> Option<ShellProcessSpec> {
    catalog
        .buff_feature_rows(buff_id)
        .into_iter()
        .find_map(|raw| {
            let fields = raw
                .split('#')
                .map(str::parse::<i32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            let [act_id, args @ ..] = fields.as_slice() else {
                return None;
            };
            let spec = process_spec_from_args(args)?;
            (catalog.buff_act_definition(*act_id).map(|act| act.kind)
                == Some(BuffActKind::ShellProcess)
                && (spec.stock_buff_id == buff_id || spec.deployed_buff_id == buff_id))
                .then_some(spec)
        })
}

pub fn deployed_buff_id(stock_buff_id: i32) -> Option<i32> {
    process_spec(stock_buff_id)
        .filter(|spec| spec.stock_buff_id == stock_buff_id)
        .map(|spec| spec.deployed_buff_id)
}

pub(crate) fn resolve_deployed_buff_id(catalog: BattleCatalog, stock_buff_id: i32) -> Option<i32> {
    resolve_process_spec(catalog, stock_buff_id)
        .filter(|spec| spec.stock_buff_id == stock_buff_id)
        .map(|spec| spec.deployed_buff_id)
}

// A fully deployed stock is removed, so the caster's shells are also found through the ones it deployed.
pub(crate) fn caster_shell_spec(
    managers: &BattleManagers,
    caster_uid: i64,
) -> Option<ShellProcessSpec> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find_map(|feature| {
            if !super::is_kind(&feature, BuffActKind::ShellProcess) {
                return None;
            }
            let spec = process_spec_from_args(feature.values.get(1..)?)?;
            let held = feature.owner_uid == caster_uid && feature.buff_id == spec.stock_buff_id;
            let deployed =
                feature.source_uid == caster_uid && feature.buff_id == spec.deployed_buff_id;
            (held || deployed).then_some(spec)
        })
}

fn runtime_process_spec(managers: &BattleManagers, buff_id: i32) -> Option<ShellProcessSpec> {
    let catalog = managers
        .buff
        .try_catalog()
        .or_else(BattleCatalog::try_global)?;
    resolve_process_spec(catalog, buff_id)
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
            // "After being attacked or sharing damage": a skill or skill-effect hit, or a ShareHurt share.
            let (attacker_uid, target_uid, damage, event_count) = match event {
                BattleEvent::Hit(hit)
                    if matches!(
                        hit.damage_from,
                        HurtDamageFromType::Skill | HurtDamageFromType::SkillEffect
                    ) =>
                {
                    (hit.source_uid, hit.target_uid, hit.amount, 1)
                }
                BattleEvent::DamageShared {
                    source_uid,
                    target_uid,
                    amount,
                    share_count,
                    ..
                } => (*source_uid, *target_uid, *amount, (*share_count).max(1)),
                _ => return Some(Vec::new()),
            };
            if target_uid != subscriber.owner_uid || damage <= 0 {
                return Some(Vec::new());
            }
            let spec = runtime_process_spec(managers, subscriber.buff_id)?;
            let amount = subscriber
                .args
                .first()
                .copied()
                .unwrap_or(1)
                .max(0)
                .saturating_mul(event_count);
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
                target_uid: attacker_uid,
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
            let spec = runtime_process_spec(managers, subscriber.buff_id)?;
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
    // Both shell buffs carry this feature; the one that just received the stacks answers, since a
    // fully deployed stock is removed.
    let receives = match change.kind {
        ShellChangeKind::Deployed => {
            subscriber.buff_id == spec.deployed_buff_id
                && subscriber.owner_uid == change.target_uid
                && subscriber.source_uid == change.source_uid
        }
        ShellChangeKind::Retrieved => {
            subscriber.buff_id == spec.stock_buff_id && subscriber.owner_uid == change.source_uid
        }
    };
    if !receives || spec.stock_buff_id != change.stock_buff_id {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    match change.kind {
        ShellChangeKind::Deployed => {
            if !determinism.roll_shell_moxie(spec.moxie_chance) || spec.moxie_delta == 0 {
                return Some(Vec::new());
            }
            Some(vec![RuleOp::Command(BattleCommand::ExPoint(
                crate::engine::manager::ex_point::ExPointCommand::Change(
                    crate::engine::manager::ex_point::ExPointChange {
                        origin,
                        source_uid: change.source_uid,
                        target_uid: change.source_uid,
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
                    let is_crit = determinism.roll_indirect_heal_crit(
                        subscriber.owner_uid,
                        ally.uid,
                        damage::crit_chance(subscriber.owner_uid, ally.uid, pool, managers),
                    );
                    let mut amount =
                        damage::modified_heal(base, subscriber.owner_uid, ally.uid, managers);
                    if is_crit {
                        amount = amount
                            * damage::crit_heal_multiplier(
                                subscriber.owner_uid,
                                ally.uid,
                                pool,
                                managers,
                            )
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
        event::{kind::EventKind, payload::ShellChangeEvent, subscription::SubscriptionKey},
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
        let catalog = BattleCatalog::try_global().unwrap();

        assert_eq!(deployed_buff_id(31090111), Some(31090112));
        assert_eq!(resolve_deployed_buff_id(catalog, 31090111), Some(31090112));
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
    fn stock_shell_preserves_shared_damage_cardinality() {
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
        let event = BattleEvent::DamageShared {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(872, "ShareHurt"),
            },
            source_uid: -1,
            target_uid: 10,
            amount: 20,
            share_count: 3,
            damage_from: HurtDamageFromType::Skill,
        };

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
                    amount: 3,
                    ..
                }
            ))]
        ));
    }

    #[test]
    fn stock_shell_deploys_after_a_skill_effect_hit_but_not_after_buff_damage() {
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
        let pool = TargetPool::from_fight(&fight);
        let deploys = |damage_from| {
            let event = BattleEvent::Hit(crate::engine::event::payload::HitEvent {
                origin: CommandOrigin {
                    domain: RuleDomain::Skill,
                    key: DefinitionKey::new(109380001, "SkillDamage"),
                },
                source_uid: -1,
                target_uid: 10,
                skill_id: 109380001,
                amount: 20,
                shield_absorbed: 0,
                career_restraint: false,
                damage_from,
                share_count: 0,
                assassinate: false,
                ignore_riposte: false,
            });
            rule_ops(
                &managers,
                &pool,
                &mut RoundDeterminism::default(),
                &subscriber(10, 10, 31090111, 870, "Shell", vec![1]),
                &event,
            )
            .unwrap()
            .len()
        };

        // "After being attacked": an attack's additional skill-effect damage is still the attack.
        assert_eq!(deploys(HurtDamageFromType::SkillEffect), 1);
        assert_eq!(deploys(HurtDamageFromType::Buff), 0);
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
    fn deployed_shell_rolls_the_configured_moxie_gain_from_the_shared_rng() {
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
        let process = |owner_uid, buff_id| {
            let mut determinism = RoundDeterminism::default();
            determinism.enqueue_permille_rolls([0]);
            rule_ops(
                &managers,
                &pool,
                &mut determinism,
                &subscriber(
                    owner_uid,
                    10,
                    buff_id,
                    869,
                    "ShellProcess",
                    vec![31090111, 31090112, 200, 1, 102, 300],
                ),
                &event,
            )
            .unwrap()
        };

        // A fully deployed stock is removed, so the shells that received the stacks answer.
        assert!(process(10, 31090111).is_empty());
        let ops = process(-1, 31090112);

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
