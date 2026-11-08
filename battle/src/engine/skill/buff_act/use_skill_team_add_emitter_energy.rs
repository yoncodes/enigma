use std::collections::HashSet;

use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        eureka::{EurekaChange, EurekaCommand},
    },
    mechanic::impromptu,
    skill::{
        action::SkillExecutionMode,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    feature: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    if feature.owner_uid != action.source_uid {
        return Some(Vec::new());
    }
    let Some((power_id, inspiration)) = spec(
        feature,
        action.mode,
        catalog.is_attack(action.skill_id),
        managers.catalog().skill_rank(action.skill_id),
    ) else {
        return Some(Vec::new());
    };
    let origin = super::command_origin(feature)?;
    let mut seen = HashSet::new();
    let features = managers.buff.active_features(&managers.hp);
    let mut ops = features
        .iter()
        .filter(|holder| {
            holder.owner_alive
                && holder.team_type == feature.team_type
                && holder.act_id().and_then(|act_id| {
                    super::registry::find(act_id, &holder.act_type).map(|definition| definition.key)
                }) == Some(feature.key.definition)
                && seen.insert(holder.owner_uid)
        })
        .map(|holder| {
            RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                origin,
                source_uid: feature.source_uid,
                target_uid: holder.owner_uid,
                power_id,
                delta: -1,
                effect_type: EffectType::Powerchange as i32,
            })))
        })
        .collect::<Vec<_>>();
    let active = features.iter().find(|active| {
        active.owner_uid == feature.owner_uid
            && active.buff_uid == feature.buff_uid
            && active.act_id().and_then(|act_id| {
                super::registry::find(act_id, &active.act_type).map(|definition| definition.key)
            }) == Some(feature.key.definition)
    })?;
    ops.push(impromptu::team_energy_gain_rule_op(
        &managers.gauge,
        active,
        inspiration,
    )?);
    Some(ops)
}

fn spec(
    feature: &BuffActSubscriber,
    mode: SkillExecutionMode,
    is_attack: bool,
    rank: i32,
) -> Option<(i32, i32)> {
    let [power_id, inspiration, ranks @ ..] = feature.args.as_slice() else {
        return None;
    };
    (super::subscriber_is_kind(
        feature,
        super::registry::BuffActKind::UseSkillTeamAddEmitterEnergy,
    ) && mode == SkillExecutionMode::Active
        && is_attack
        && (ranks.is_empty() || ranks.contains(&rank))
        && *power_id > 0
        && *inspiration > 0)
        .then_some((*power_id, *inspiration))
}

#[cfg(test)]
#[path = "use_skill_team_add_emitter_energy/tests.rs"]
mod tests;
