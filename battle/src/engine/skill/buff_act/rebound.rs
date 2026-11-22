use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{DamageFormulaInput, calculate, modifiers},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers, HpExecution,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::{
            DamageEffectKind, HpCommand, HpCommandError, HpDamage, HpLoss, HurtDamageFromType,
            HurtInfoData,
        },
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReflectedDamage(HpDamage);

impl ReflectedDamage {
    pub(crate) fn settle(
        self,
        managers: &mut BattleManagers,
    ) -> Result<Option<HpExecution>, HpCommandError> {
        let command = HpCommand::Damage(self.0);
        if managers.hp.current(self.0.target_uid) <= 0 {
            managers.hp.validate_command(command)?;
            return Ok(None);
        }
        let command = super::fixed_hurt::resolve_command(&managers.buff, &managers.hp, command);
        managers.execute_rule_hp(command).map(Some)
    }
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [0 | 2, permille, raw_attr]
        if *permille > 0 && AttrId::from_raw(*raw_attr).is_some())
}

pub fn supports_damage_based(args: &[i32]) -> bool {
    matches!(args, [permille, 0, 0] if *permille > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [_, permille, raw_attr] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports(&subscriber.args)
        || hit.target_uid != subscriber.owner_uid
        || hit.source_uid == 0
        || managers
            .buff
            .has_buff_act_kind(hit.source_uid, super::registry::BuffActKind::IgnoreRebound)
        || !subscriber.owner_alive
    {
        return Some(Vec::new());
    }
    let amount = calculate(DamageFormulaInput::genesis(
        managers.origin_attribute(subscriber.owner_uid, AttrId::from_raw(*raw_attr)?),
        *permille,
        modifiers::genesis_multiplier(managers, subscriber.owner_uid, hit.source_uid),
    ));
    if amount <= 0 {
        return Some(Vec::new());
    }
    reflection_ops(
        managers,
        subscriber,
        BattleCommand::ReboundDamage(ReflectedDamage(HpDamage {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.owner_uid,
            target_uid: hit.source_uid,
            amount,
            config_effect: 0,
            effect_kind: DamageEffectKind::Genesis,
            assassinate: false,
            ignore_riposte: false,
            hurt: reflected_hurt(subscriber, amount),
        })),
        DepletedBuff::Keep,
    )
}

pub fn damage_based_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    let [permille, 0, 0] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports_damage_based(&subscriber.args)
        || hit.amount <= 0
        || hit.target_uid != subscriber.owner_uid
        || hit.source_uid == 0
        || managers
            .buff
            .has_buff_act_kind(hit.source_uid, super::registry::BuffActKind::IgnoreRebound)
        || !subscriber.owner_alive
    {
        return Some(Vec::new());
    }
    let amount = calculate(DamageFormulaInput::genesis(
        hit.amount,
        *permille,
        modifiers::genesis_multiplier(managers, subscriber.owner_uid, hit.source_uid),
    ))
    .max(1);
    reflection_ops(
        managers,
        subscriber,
        BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin: super::command_origin(subscriber)?,
            source_uid: subscriber.owner_uid,
            target_uid: hit.source_uid,
            amount,
            config_effect: 0,
            hurt: Some(reflected_hurt(subscriber, amount)),
        })),
        DepletedBuff::Remove,
    )
}

fn reflected_hurt(subscriber: &BuffActSubscriber, amount: i32) -> HurtInfoData {
    HurtInfoData {
        from_uid: subscriber.owner_uid,
        is_crit: false,
        career_restraint: false,
        reduce_hp: 0,
        effect_id: 0,
        skill_id: 0,
        damage_from: HurtDamageFromType::Buff,
        buff_act_id: subscriber.key.definition.opcode,
        buff_uid: subscriber.buff_uid,
        hurt_effect_type: EffectType::Origindamage as i32,
        display_amount: Some(amount),
    }
}

fn reflection_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    damage: BattleCommand,
    depleted: DepletedBuff,
) -> Option<Vec<RuleOp>> {
    let mut ops = vec![RuleOp::Command(damage)];
    if managers.catalog().buff_has_effect_count(subscriber.buff_id) {
        let consume = BuffConsume {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted,
        };
        ops.push(RuleOp::Command(BattleCommand::Buff(
            if matches!(depleted, DepletedBuff::Remove) {
                BuffCommand::ConsumeCount(consume)
            } else {
                BuffCommand::ConsumeEffectCount(consume)
            },
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
