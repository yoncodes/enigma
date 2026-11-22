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
            let amount = subscriber.args.first().copied().unwrap_or(1).max(0);
            if amount == 0
                || managers
                    .buff
                    .buff_id_amount(subscriber.owner_uid, spec.stock_buff_id)
                    <= 0
            {
                return Some(Vec::new());
            }
            return Some(
                (0..event_count)
                    .map(|_| {
                        RuleOp::Command(BattleCommand::Shell(ShellCommand::Deploy {
                            origin,
                            source_uid: subscriber.owner_uid,
                            target_uid: attacker_uid,
                            stock_buff_id: spec.stock_buff_id,
                            amount,
                        }))
                    })
                    .collect(),
            );
        }
        BuffActKind::ShellDebuff => {
            let BattleEvent::Hit(hit) = event else {
                return Some(Vec::new());
            };
            if hit.target_uid != subscriber.owner_uid
                || hit.amount <= 0
                || !matches!(
                    hit.damage_from,
                    HurtDamageFromType::Skill | HurtDamageFromType::SkillEffect
                )
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
            let basis = managers
                .origin_attribute(subscriber.owner_uid, attr_id)
                .max(0);
            let base_numerator = i128::from(basis)
                * i128::from(spec.heal_rate.max(0))
                * i128::from(change.transaction_amount);
            let base = (base_numerator / 1000).clamp(0, i128::from(i32::MAX)) as i32;
            if base <= 0 {
                return Some(Vec::new());
            }
            let heals = pool
                .main_allies(subscriber.owner_uid)
                .iter()
                .filter(|ally| managers.hp.current(ally.uid) > 0)
                .map(|ally| {
                    let is_crit = determinism.roll_indirect_heal_crit(
                        spec.deployed_buff_id,
                        subscriber.owner_uid,
                        ally.uid,
                        damage::crit_chance(subscriber.owner_uid, ally.uid, pool, managers),
                    );
                    let multiplier = if is_crit {
                        damage::crit_heal_multiplier(subscriber.owner_uid, ally.uid, pool, managers)
                    } else {
                        damage::CriticalHealMultiplier::IDENTITY
                    };
                    let amount = damage::modified_fractional_heal(
                        i128::from(base),
                        1,
                        subscriber.owner_uid,
                        ally.uid,
                        managers,
                        multiplier,
                    );
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

pub(crate) fn linked_damage_inherits_attribute(extra_skill_kind: i32, attr_id: AttrId) -> bool {
    attr_id == AttrId::DmgBonus
        || (attr_id == AttrId::CriticalDmg
            && matches!(
                crate::engine::skill::condition::extra::skill_kind_from_is_extra(extra_skill_kind),
                Some(crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction)
            ))
}

#[cfg(test)]
mod tests;
