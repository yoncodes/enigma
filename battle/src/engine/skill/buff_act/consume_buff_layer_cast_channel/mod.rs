use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{
            BuffCommand, BuffConsume, BuffRemove, BuffRemoveSelector, BuffSelector, DepletedBuff,
        },
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        buff_act::registry::BuffActKind,
        condition::extra::skill_kind_from_is_extra,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

const SKILL_OWNED_TARGET_RULE: i32 = 202;

pub fn parse(raw_args: &[String]) -> Option<Vec<i32>> {
    let [resource_buff_id, target_rule, tiers @ ..] = raw_args else {
        return None;
    };
    let resource_buff_id = resource_buff_id.parse::<i32>().ok().filter(|id| *id > 0)?;
    let target_rule = target_rule
        .parse::<i32>()
        .ok()
        .filter(|rule| *rule == SKILL_OWNED_TARGET_RULE)?;
    let mut parsed = vec![resource_buff_id, target_rule, tiers.len() as i32];
    let mut previous_threshold = -1;
    for tier in tiers {
        let values = tier
            .split(',')
            .map(str::trim)
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let (&threshold, skills) = values.split_first()?;
        if threshold <= previous_threshold || skills.is_empty() || skills.iter().any(|id| *id <= 0)
        {
            return None;
        }
        previous_threshold = threshold;
        parsed.extend([threshold, skills.len() as i32]);
        parsed.extend(skills);
    }
    (previous_threshold >= 0).then_some(parsed)
}

pub fn supports(args: &[i32]) -> bool {
    decode(args).is_some()
}

pub fn referenced_buff(args: &[i32]) -> Option<i32> {
    decode(args).map(|spec| spec.resource_buff_id)
}

pub fn referenced_skills(args: &[i32]) -> impl Iterator<Item = i32> + '_ {
    decode(args).into_iter().flat_map(|spec| {
        spec.tiers
            .into_iter()
            .flat_map(|tier| tier.skills.iter().copied())
    })
}

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::ConsumeBuffLayerCastChannel)
        || event.kind() != EventKind::SmallRoundEnd
    {
        return None;
    }
    let spec = decode(&subscriber.args)?;
    let available = managers
        .buff
        .buff_id_or_type_amount(subscriber.owner_uid, spec.resource_buff_id)
        .max(0);
    let tier = spec
        .tiers
        .iter()
        .rev()
        .find(|tier| tier.threshold <= available)?;
    let consumed = tier.threshold;
    let origin = super::command_origin(subscriber)?;
    let mut ops = Vec::with_capacity(tier.skills.len() + 2);
    if consumed > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::IdOrType(spec.resource_buff_id),
                amount: consumed,
                depleted: DepletedBuff::Remove,
            },
        ))));
    }
    for &skill_id in tier.skills {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: subscriber.owner_uid,
            skill_id,
        }
        .into();
        invocation.mode = SkillExecutionMode::Active;
        invocation.extra_skill_kind = skill_kind_from_is_extra(catalog.extra_kind(skill_id));
        ops.push(RuleOp::Skill(invocation));
    }
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
        BuffRemove {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
        },
    ))));
    Some(ops)
}

struct Spec<'a> {
    resource_buff_id: i32,
    tiers: Vec<Tier<'a>>,
}

struct Tier<'a> {
    threshold: i32,
    skills: &'a [i32],
}

fn decode(args: &[i32]) -> Option<Spec<'_>> {
    let [resource_buff_id, target_rule, tier_count, tail @ ..] = args else {
        return None;
    };
    if *resource_buff_id <= 0 || *target_rule != SKILL_OWNED_TARGET_RULE || *tier_count <= 0 {
        return None;
    }
    let mut tiers = Vec::with_capacity(*tier_count as usize);
    let mut remaining = tail;
    let mut previous_threshold = -1;
    while let [threshold, skill_count, rest @ ..] = remaining {
        let skill_count = usize::try_from(*skill_count)
            .ok()
            .filter(|count| *count > 0)?;
        let (skills, rest) = rest.split_at_checked(skill_count)?;
        if *threshold <= previous_threshold || skills.iter().any(|id| *id <= 0) {
            return None;
        }
        tiers.push(Tier {
            threshold: *threshold,
            skills,
        });
        previous_threshold = *threshold;
        remaining = rest;
    }
    (remaining.is_empty() && tiers.len() == *tier_count as usize && tiers[0].threshold == 0)
        .then_some(Spec {
            resource_buff_id: *resource_buff_id,
            tiers,
        })
}

#[cfg(test)]
mod tests;
