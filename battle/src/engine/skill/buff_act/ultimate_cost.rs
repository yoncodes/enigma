use sonettobuf::effect_type_enum::EffectType;

use super::{
    ActiveBuffFeature, BattleEvent, BattleManagers, RuleOp, changed_features, feature_kind,
    registry::{BuffActDefinition, BuffActKind},
};

pub(super) fn on_grant(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::BuffAdded(change) = event else {
        return Vec::new();
    };
    changed_features(managers, event, BuffActKind::ExSkillPointChange)
        .into_iter()
        .filter_map(|(feature, _)| {
            let [_, offset] = feature.values.as_slice() else {
                return None;
            };
            if !managers
                .buff
                .is_effective(change.target_uid, change.buff_uid)
                || managers
                    .buff
                    .is_fanout_instance(change.target_uid, change.buff_uid)
            {
                return None;
            }
            let op = RuleOp::BuffFeatureMarker {
                target_uid: change.target_uid,
                effect_type: EffectType::Exskillpointchange as i32,
                effect_num: *offset,
                buff_act_id: 0,
            };
            Some((feature, op))
        })
        .collect()
}

pub(super) fn cost_precedes_cap(managers: &BattleManagers, event: &BattleEvent) -> bool {
    let BattleEvent::BuffAdded(change) = event else {
        return false;
    };
    let features = managers.buff.definition_features(change.buff_id);
    let position = |kind| {
        features
            .iter()
            .position(|feature| feature_kind(feature) == Some(kind))
    };
    matches!(
        (position(BuffActKind::ExSkillPointChange), position(BuffActKind::ExPointMaxAdd)),
        (Some(cost), Some(cap)) if cost < cap
    )
}

pub(super) fn order_grant(definitions: &mut Vec<&BuffActDefinition>) {
    let position = |kind| {
        definitions
            .iter()
            .position(|definition| definition.kind == kind)
    };
    if let (Some(cost), Some(cap)) = (
        position(BuffActKind::ExSkillPointChange),
        position(BuffActKind::ExPointMaxAdd),
    ) && cap < cost
    {
        let definition = definitions.remove(cost);
        definitions.insert(cap, definition);
    }
}
