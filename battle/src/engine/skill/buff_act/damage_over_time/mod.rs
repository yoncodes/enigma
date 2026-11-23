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
    ) || matches!(
        args,
        [1, current_hp, rate, 0, max_hp, cap]
            if AttrId::from_raw(*current_hp) == Some(AttrId::CurrentHp)
                && AttrId::from_raw(*max_hp) == Some(AttrId::Hp)
                && *rate > 0
                && *cap > 0
    )
}

pub fn supports_round_end_dot(args: &[i32]) -> bool {
    supports_dot(args)
        || matches!(
            args,
            [0, attr_id, permille] if *attr_id == AttrId::Hp.id() && *permille > 0
        )
}

/// Arguments of every active act of `kind` held by `owner_uid`.
fn holder_acts(
    managers: &BattleManagers,
    owner_uid: i64,
    kind: BuffActKind,
) -> impl Iterator<Item = Vec<i32>> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(move |feature| feature.owner_uid == owner_uid && buff_act::is_kind(feature, kind))
        .map(|feature| feature.values.into_iter().skip(1).collect())
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
    if let Some(effect_type) =
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
    if matches!(kind, DamageOverTimeKind::Burn)
        && holder_acts(managers, subscriber.owner_uid, BuffActKind::ImmunityBurn)
            .next()
            .is_some()
    {
        return None;
    }
    let origin = super::command_origin(subscriber)?;
    let source_uid = if subscriber.source_uid != 0 {
        subscriber.source_uid
    } else {
        subscriber.owner_uid
    };
    let mut amount = match (kind, subscriber.args.as_slice()) {
        (DamageOverTimeKind::Poison, _) if remaining_duration => {
            managers.buff.dot_remaining_damage_for_owner(
                subscriber.owner_uid,
                subscriber.key.definition.opcode,
                subscriber.buff_uid,
            )?
        }
        (DamageOverTimeKind::Dot, [0, ..]) => managers.buff.dot_damage_for_owner(
            subscriber.owner_uid,
            subscriber.key.definition.opcode,
            subscriber.buff_uid,
        )?,
        (DamageOverTimeKind::Poison, _) => managers.buff.dot_damage_for_owner(
            subscriber.owner_uid,
            subscriber.key.definition.opcode,
            subscriber.buff_uid,
        )?,
        (DamageOverTimeKind::Dot, [1, current_hp, rate, 0, max_hp, cap]) => {
            attribute_scaled_damage(
                effective_attribute(
                    managers,
                    subscriber.owner_uid,
                    AttrId::from_raw(*current_hp)?,
                ),
                *rate,
                subscriber.amount.max(1),
            )
            .min(attribute_scaled_damage(
                effective_attribute(managers, source_uid, AttrId::from_raw(*max_hp)?),
                *cap,
                subscriber.amount.max(1),
            ))
        }
        (
            DamageOverTimeKind::Burn | DamageOverTimeKind::Dot | DamageOverTimeKind::Radiance,
            [mode, attr_id, permille],
        ) => {
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
        _ => return None,
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
    if matches!(kind, DamageOverTimeKind::Burn)
        && holder_acts(managers, subscriber.owner_uid, BuffActKind::LockBurn)
            .any(|args| args == [subscriber.buff_id])
    {
        return None;
    }
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current = buff.layer.unwrap_or_default();
    let next = settled_layer(kind, &subscriber.args, current)?;
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

fn settled_layer(kind: DamageOverTimeKind, args: &[i32], current: i32) -> Option<i32> {
    match kind {
        DamageOverTimeKind::Burn | DamageOverTimeKind::Radiance => Some(current / 2),
        DamageOverTimeKind::Dot if matches!(args, [0, attr_id, _] if AttrId::from_raw(*attr_id) == Some(AttrId::Hp)) => {
            Some(current / 2)
        }
        DamageOverTimeKind::Dot => None,
        DamageOverTimeKind::Poison => None,
    }
}

#[cfg(test)]
mod tests;
