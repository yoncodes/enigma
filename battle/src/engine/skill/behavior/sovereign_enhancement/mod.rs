use crate::engine::{
    entity::attr::AttrId,
    manager::card::SovereignEnhancement,
    skill::{
        action::SkillRateModifier,
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::RuleOp,
    },
};

pub(super) struct Handler;

enum Enhancement {
    Damage { main: i32, all: i32 },
    Targets { count: i32, rate: i32 },
    Attributes([(AttrId, i32); 2]),
}

fn nonnegative(raw: &str) -> Option<i32> {
    raw.parse::<i32>().ok().filter(|value| *value >= 0)
}

fn parse(behavior: &ParsedBehavior) -> Option<(SovereignEnhancement, Enhancement)> {
    let [kind, payload] = behavior.raw_args.as_slice() else {
        return None;
    };
    let (mode, values) = payload.split_once(':')?;
    Some(match (kind.as_str(), mode) {
        ("1", "2") | ("2", "3") => {
            let (first, all) = values.split_once("&1:")?;
            let first = nonnegative(first)?;
            let all = nonnegative(all)?;
            if kind == "1" {
                (
                    SovereignEnhancement::Single,
                    Enhancement::Damage { main: first, all },
                )
            } else {
                (
                    SovereignEnhancement::Mass,
                    Enhancement::Targets {
                        count: first,
                        rate: all,
                    },
                )
            }
        }
        ("3", "4") | ("4", "4") => {
            let (first, second) = values.split_once('|')?;
            let pair = |raw: &str| {
                let (attr, value) = raw.split_once('_')?;
                Some((AttrId::from_raw(attr.parse().ok()?)?, nonnegative(value)?))
            };
            let attributes = [pair(first)?, pair(second)?];
            let kind = match (kind.as_str(), attributes[0].0, attributes[1].0) {
                ("3", AttrId::CriticalRate, AttrId::UltimateMight) => SovereignEnhancement::Support,
                ("4", AttrId::CriticalDmg, AttrId::IncantationMight) => {
                    SovereignEnhancement::Recovery
                }
                _ => return None,
            };
            (kind, Enhancement::Attributes(attributes))
        }
        _ => return None,
    })
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        parse(behavior).is_some()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let (kind, enhancement) = parse(behavior)?;
        let level = i32::from(
            context
                .managers
                .card
                .sovereign_level(
                    context.source_uid,
                    context.active_skill_id,
                    context.target.active_card_index,
                    kind,
                )
                .ok()?,
        );
        if level == 0 {
            return Some(Vec::new());
        }
        let main_target = context.target.runtime_target_uid;
        if main_target == 0 || context.target_uid != context.source_uid {
            return None;
        }
        let mut ops = Vec::new();
        let mut rate = |target_uid, delta: i32| {
            context.modifiers.rates.push(SkillRateModifier::fixed(
                target_uid,
                behavior.spec.key.opcode,
                delta.saturating_mul(level),
                true,
            ));
        };
        match enhancement {
            Enhancement::Damage { main, all } => {
                rate(main_target, main);
                rate(0, all);
            }
            Enhancement::Targets { count, rate: delta } => {
                rate(0, delta);
                ops.push(RuleOp::ModifyActiveSkillTargets {
                    additional_count: count.saturating_mul(level),
                });
            }
            Enhancement::Attributes(attributes) => {
                context.modifiers.attack_attributes.extend(
                    attributes
                        .into_iter()
                        .map(|(attr, value)| (attr, value.saturating_mul(level))),
                );
            }
        }
        ops.push(RuleOp::EffectMarker {
            target_uid: main_target,
            effect_type: sonettobuf::effect_type_enum::EffectType::Unnamedstrengthen as i32,
            effect_num: behavior.arg(0)?,
            effect_num1: Some(level),
            config_effect: 0,
            reserve_id: None,
            reserve_str: None,
        });
        Some(ops)
    }
}

#[cfg(test)]
mod tests;
