use sonettobuf::BuffActInfo;

use crate::engine::{
    damage::scale_permille,
    entity::attr::AttrId,
    event::payload::{BattleEvent, HealCast},
    skill::{
        buff_act::{
            BuffActRuleOp,
            registry::{BuffActKind, ParsedBuffAct, RuntimeContext, resolve_feature},
        },
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

mod accrual;

pub use accrual::Accrue;

struct Rule<'a> {
    base: i32,
    capacity: i32,
    receiver: i32,
    bonus: i32,
    skills: &'a [i32],
}

impl<'a> Rule<'a> {
    fn parse(args: &'a [i32]) -> Option<Self> {
        let [base, attr, capacity, receiver, bonus, skills @ ..] = args else {
            return None;
        };
        (*base > 0
            && AttrId::from_raw(*attr) == Some(AttrId::Attack)
            && *capacity > 0
            && *receiver > 0
            && *bonus > 0
            && !skills.is_empty()
            && skills.iter().all(|id| *id > 0))
        .then_some(Self {
            base: *base,
            capacity: *capacity,
            receiver: *receiver,
            bonus: *bonus,
            skills,
        })
    }

    fn amount(&self, heal: i32, action_skill: Option<i32>) -> i32 {
        let bonus = action_skill
            .filter(|skill_id| self.skills.contains(skill_id))
            .map_or(0, |_| self.bonus);
        scale_permille(heal, self.base.saturating_add(bonus))
    }
}

pub(crate) fn parse_args(raw: &[String]) -> Option<Vec<i32>> {
    let [base, attr, capacity, receiver, skills, bonus] = raw else {
        return None;
    };
    let mut args = [base, attr, capacity, receiver, bonus]
        .into_iter()
        .map(|number| number.parse().ok())
        .collect::<Option<Vec<_>>>()?;
    args.extend(
        skills
            .split(',')
            .map(|id| id.parse::<i32>().ok())
            .collect::<Option<Vec<_>>>()?,
    );
    supports(&args).then_some(args)
}

pub(crate) fn supports(args: &[i32]) -> bool {
    Rule::parse(args).is_some()
}

pub(crate) fn supports_link(game: Option<&config::GameDB>, raw: &str) -> bool {
    let Some(game) = game else {
        return false;
    };
    raw.split('#')
        .nth(4)
        .and_then(|receiver| receiver.parse().ok())
        .is_some_and(|receiver| receiver_act(game, receiver).is_some())
}

fn receiver_act(game: &config::GameDB, receiver: i32) -> Option<i32> {
    let receiver = game.skill_buff.get(receiver)?;
    receiver.features.split('|').find_map(|raw| {
        let opcode = raw.split('#').next()?.trim().parse().ok()?;
        let act = game.buff_act.get(opcode)?;
        if super::registry::kind(opcode, &act.r#type) != Some(BuffActKind::RippleBalance) {
            return None;
        }
        let feature = resolve_feature(Some(game), raw)?;
        feature.arguments_supported.then_some(opcode)
    })
}

pub(crate) fn references(_: Option<&config::GameDB>, feature: &ParsedBuffAct) -> RuleReferences {
    RuleReferences {
        buffs: Rule::parse(&feature.args)
            .map(|rule| rule.receiver)
            .into_iter()
            .collect(),
        ..RuleReferences::default()
    }
}

pub(crate) fn initial_act_info(source_attack: i32, values: &[i32]) -> Option<BuffActInfo> {
    let (act_id, args) = values.split_first()?;
    let rule = Rule::parse(args)?;
    Some(BuffActInfo {
        act_id: Some(*act_id),
        param: vec![scale_permille(source_attack, rule.capacity)],
        str_param: Some(String::new()),
    })
}
pub(crate) fn rule_ops(context: &mut RuntimeContext<'_>) -> Option<Vec<BuffActRuleOp>> {
    let subscriber = context.subscriber;
    let BattleEvent::HealResolved(heal) = context.event? else {
        return Some(Vec::new());
    };
    if !subscriber.owner_alive || heal.source_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let rule = Rule::parse(&subscriber.args)?;
    let amount = rule.amount(heal.amount, heal.cast.map(HealCast::skill_id));
    if amount <= 0 {
        return Some(Vec::new());
    }
    let carrier = context
        .managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let capacity = carrier.act_info.iter().find_map(|info| {
        let [capacity] = info.param.as_slice() else {
            return None;
        };
        (info.act_id == Some(subscriber.key.definition.opcode)).then_some(*capacity)
    })?;
    if capacity <= 0 {
        return Some(Vec::new());
    }
    let act_id = receiver_act(context.managers.catalog().game_data(), rule.receiver)?;
    let op = RuleOp::Command(BattleCommand::RippleAccrual(Accrue {
        origin: super::command_origin(subscriber)?,
        source_uid: heal.source_uid,
        target_uid: heal.target_uid,
        receiver: rule.receiver,
        act_id,
        amount,
        capacity,
    }));
    let impact = match heal.cast {
        Some(HealCast::Reaction(_)) => BuffActRuleOp::causing_parent(op),
        _ => BuffActRuleOp::causing(op),
    };
    Some(vec![impact])
}

#[cfg(test)]
mod tests;
