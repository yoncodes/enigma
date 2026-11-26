use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    damage::{
        DamageFormulaInput, calculate_with_trace_for_version,
        handler::{crit_chance, crit_damage_multiplier},
        modifiers::periodic_genesis_multiplier,
    },
    entity::attr::AttrId,
    manager::hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    skill::{
        buff_act::registry::{ParsedBuffAct, RuntimeContext},
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [rate, buff_id] if *rate > 0 && *buff_id > 0)
}

pub fn references(_: Option<&config::GameDB>, feature: &ParsedBuffAct) -> RuleReferences {
    RuleReferences {
        buffs: feature.args.get(1).copied().into_iter().collect(),
        ..RuleReferences::default()
    }
}

pub fn rule_ops(context: &mut RuntimeContext<'_>) -> Option<Vec<RuleOp>> {
    let subscriber = context.subscriber;
    let managers = context.managers;
    let [rate, buff_id] = subscriber.args.as_slice() else {
        return None;
    };
    let source_uid = subscriber.source_uid;
    let target_uid = subscriber.owner_uid;
    let source_team = context.pool.team_type(source_uid)?;
    let layers = managers.buff.buff_id_amount(target_uid, *buff_id);
    let spent = managers.ex_point.round_ultimate_spend(source_team);
    if !subscriber.owner_alive || managers.hp.current(source_uid) <= 0 || layers <= 0 || spent <= 0
    {
        return Some(Vec::new());
    }

    let attack = managers.origin_attribute(source_uid, AttrId::Attack);
    let base_rate = rate.saturating_mul(layers).saturating_mul(spent);
    let mut formula = DamageFormulaInput::genesis(
        attack,
        base_rate,
        periodic_genesis_multiplier(managers, source_uid, target_uid),
    );
    formula.might =
        1_000i32.saturating_add(managers.origin_attribute(source_uid, AttrId::UltimateMight));
    formula.is_crit = context.determinism.roll_hidden_crit(
        subscriber.buff_id,
        source_uid,
        target_uid,
        crit_chance(source_uid, target_uid, context.pool, managers),
    );
    if formula.is_crit {
        formula.crit_multiplier =
            crit_damage_multiplier(source_uid, target_uid, context.pool, managers);
    }
    let amount = calculate_with_trace_for_version(formula, managers.fight_version()).amount;
    if amount <= 0 {
        return Some(Vec::new());
    }
    let hurt_effect = if formula.is_crit {
        EffectType::Origincrit
    } else {
        EffectType::Origindamage
    };
    Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
        HpLoss {
            origin: super::command_origin(subscriber)?,
            source_uid,
            target_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: source_uid,
                is_crit: formula.is_crit,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: hurt_effect as i32,
                display_amount: Some(amount),
            }),
        },
    )))])
}

#[cfg(test)]
mod tests;
