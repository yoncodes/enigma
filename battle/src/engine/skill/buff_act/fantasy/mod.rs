use crate::engine::{
    damage::formula::scale_permille_stacks,
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
        emitter,
        gauge::{GaugeCommand, GaugeOperation},
        hp::{HpCommand, HpHeal, HpHealKind},
    },
    mechanic::impromptu,
    skill::{
        buff_act::registry::{BuffActKind, ParsedBuffAct, RuntimeContext},
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetRequest, TargetResolver},
    },
};

use super::BuffActRuleOp;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    source_buff: i32,
    cap: i32,
    debuff_target: i32,
    debuff: i32,
    bonus_target: i32,
    bonuses: Vec<i32>,
    heal_target: i32,
    heal_mode: i32,
    heal_attr: AttrId,
    heal_rate: i32,
    inspiration: i32,
}

impl Rule {
    fn from_args(args: &[i32]) -> Option<Self> {
        let [
            source_buff,
            cap,
            debuff_target,
            debuff,
            bonus_target,
            rest @ ..,
        ] = args
        else {
            return None;
        };
        let cap = usize::try_from(*cap).ok()?;
        if cap == 0 || rest.len() != cap + 5 {
            return None;
        }
        let (bonuses, tail) = rest.split_at(cap);
        let [heal_target, heal_mode, heal_attr, heal_rate, inspiration] = tail else {
            return None;
        };
        let rule = Self {
            source_buff: *source_buff,
            cap: i32::try_from(cap).ok()?,
            debuff_target: *debuff_target,
            debuff: *debuff,
            bonus_target: *bonus_target,
            bonuses: bonuses.to_vec(),
            heal_target: *heal_target,
            heal_mode: *heal_mode,
            heal_attr: AttrId::from_raw(*heal_attr)?,
            heal_rate: *heal_rate,
            inspiration: *inspiration,
        };
        (rule.source_buff > 0
            && rule.debuff_target > 0
            && rule.debuff > 0
            && rule.bonus_target > 0
            && rule.bonuses.iter().all(|buff| *buff > 0)
            && rule.heal_target > 0
            && rule.heal_mode == 1
            && rule.heal_rate > 0
            && rule.inspiration > 0)
            .then_some(rule)
    }
}

pub fn parse(raw: &[String]) -> Option<Vec<i32>> {
    let [
        source_buff,
        cap,
        debuff_target,
        debuff,
        bonus_target,
        bonuses,
        heal_target,
        heal_mode,
        heal_attr,
        heal_rate,
        inspiration,
    ] = raw
    else {
        return None;
    };
    let scalar = |value: &String| value.trim().parse::<i32>().ok();
    let cap = scalar(cap)?;
    let bonuses = bonuses
        .split(',')
        .map(str::trim)
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let values = [
        scalar(source_buff)?,
        cap,
        scalar(debuff_target)?,
        scalar(debuff)?,
        scalar(bonus_target)?,
    ]
    .into_iter()
    .chain(bonuses)
    .chain([
        scalar(heal_target)?,
        scalar(heal_mode)?,
        scalar(heal_attr)?,
        scalar(heal_rate)?,
        scalar(inspiration)?,
    ])
    .collect::<Vec<_>>();
    Rule::from_args(&values).map(|_| values)
}

pub fn supports(args: &[i32]) -> bool {
    Rule::from_args(args).is_some()
}

pub fn references(_: Option<&config::GameDB>, feature: &ParsedBuffAct) -> RuleReferences {
    let Some(rule) = Rule::from_args(&feature.args) else {
        return RuleReferences::default();
    };
    RuleReferences {
        buffs: std::iter::once(rule.source_buff)
            .chain(std::iter::once(rule.debuff))
            .chain(rule.bonuses)
            .collect(),
        ..RuleReferences::default()
    }
}

pub fn rule_ops(context: &mut RuntimeContext<'_>) -> Option<Vec<BuffActRuleOp>> {
    let subscriber = context.subscriber;
    if !super::subscriber_is_kind(subscriber, BuffActKind::ToothFairyFantasy)
        || context.event?.kind() != EventKind::RoundStart
    {
        return None;
    }
    let rule = Rule::from_args(&subscriber.args)?;
    if !subscriber.owner_alive {
        return Some(Vec::new());
    }
    let consumed = context
        .managers
        .buff
        .buff_id_amount(subscriber.owner_uid, rule.source_buff)
        .min(rule.cap);
    if consumed <= 0 {
        return Some(Vec::new());
    }

    let origin = super::command_origin(subscriber)?;
    let resolve = |code, determinism: &mut _| {
        TargetResolver::resolve_with_managers_and_context(
            &TargetRequest {
                code,
                raw: Vec::new(),
            },
            0,
            subscriber.owner_uid,
            context.pool,
            determinism,
            Some(context.managers),
            TargetContext::default(),
        )
    };
    let debuff_targets = resolve(rule.debuff_target, context.determinism);
    let bonus_targets = resolve(rule.bonus_target, context.determinism);
    let heal_targets = resolve(rule.heal_target, context.determinism);

    let mut ops = vec![BuffActRuleOp::subscriber(RuleOp::Command(
        BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::ExactId(rule.source_buff),
            amount: consumed,
            depleted: DepletedBuff::RemoveWithZeroLayer,
        })),
    ))];
    ops.extend(debuff_targets.into_iter().map(|target_uid| {
        BuffActRuleOp::causing(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid,
                buff_id: rule.debuff,
                amount: Some(consumed),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))))
    }));
    for buff_id in rule.bonuses.into_iter().take(consumed as usize) {
        ops.extend(bonus_targets.iter().map(|target_uid| {
            BuffActRuleOp::causing(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    origin,
                    source_uid: subscriber.owner_uid,
                    target_uid: *target_uid,
                    buff_id,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))))
        }));
    }
    ops.extend(heal_targets.into_iter().filter_map(|target_uid| {
        let base = scale_permille_stacks(
            context
                .managers
                .origin_attribute(target_uid, rule.heal_attr),
            rule.heal_rate,
            consumed,
        );
        let amount = crate::engine::damage::handler::modified_heal(
            base,
            subscriber.owner_uid,
            target_uid,
            context.managers,
        );
        (amount > 0).then(|| {
            BuffActRuleOp::causing(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
                HpHeal {
                    origin,
                    source_uid: subscriber.owner_uid,
                    target_uid,
                    amount,
                    config_effect: subscriber.key.definition.opcode,
                    kind: HpHealKind::Normal,
                },
            ))))
        })
    }));
    ops.push(BuffActRuleOp::causing(RuleOp::Command(
        BattleCommand::Gauge(
            GaugeCommand::new(
                origin,
                impromptu::inspiration_key(emitter::UID),
                GaugeOperation::ChangeValue {
                    delta: consumed.saturating_mul(rule.inspiration),
                },
            )
            .attributed_to(subscriber.owner_uid, subscriber.key.definition.opcode),
        ),
    )));
    Some(ops)
}

#[cfg(test)]
mod tests;
