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
    let consumed = available.min(spec.tiers.last()?.threshold);
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
mod tests {
    use super::*;
    use crate::engine::skill::{rule::DefinitionKey, subscriber::BuffActSubscriber};

    const FEATURE: &str = "31350131#202#0,31350141#10,31350142#20,31350143,31350141#30,31350143,31350142#40,31350143,31350143,31350141#50,31350143,31350143,31350142#60,31350143,31350143,31350143";

    #[test]
    fn structured_tiers_preserve_thresholds_and_skill_lists() {
        let parsed = parse(&FEATURE.split('#').map(str::to_owned).collect::<Vec<_>>()).unwrap();
        let spec = decode(&parsed).unwrap();

        assert_eq!(spec.resource_buff_id, 31350131);
        assert_eq!(spec.tiers.len(), 7);
        assert_eq!(spec.tiers[3].threshold, 30);
        assert_eq!(spec.tiers[3].skills, [31350143, 31350142]);
        assert_eq!(spec.tiers[6].skills, [31350143, 31350143, 31350143]);

        let mut unsupported = FEATURE.split('#').map(str::to_owned).collect::<Vec<_>>();
        unsupported[1] = "203".to_owned();
        assert!(parse(&unsupported).is_none());
    }

    #[test]
    fn round_end_consumes_available_resource_casts_the_selected_tier_and_ends_channel() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31350131),
                        layer: Some(35),
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
            buff_uid: 2,
            buff_id: 31350121,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                EventKind::SmallRoundEnd,
                DefinitionKey::new(1054, "ConsumeBuffLayerCastChannel"),
            ),
            act_type: "ConsumeBuffLayerCastChannel".to_owned(),
            effect_time: 301,
            effect_condition: 0,
            args: parse(&FEATURE.split('#').map(str::to_owned).collect::<Vec<_>>()).unwrap(),
            raw: format!("1054#{FEATURE}"),
        };

        let ops = rule_ops(
            &managers,
            &SkillEffectCatalog::from_game_db(config::configs::get()),
            &subscriber,
            &BattleEvent::Kind(EventKind::SmallRoundEnd),
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                    amount: 35,
                    ..
                }))),
                RuleOp::Skill(SkillInvocation {
                    plan: SkillRequest {
                        skill_id: 31350143,
                        ..
                    },
                    ..
                }),
                RuleOp::Skill(SkillInvocation {
                    plan: SkillRequest {
                        skill_id: 31350142,
                        ..
                    },
                    ..
                }),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    selector: BuffRemoveSelector::Uid(2),
                    ..
                })))
            ]
        ));
    }
}
