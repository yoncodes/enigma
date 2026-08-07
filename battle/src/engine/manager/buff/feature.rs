use sonettobuf::BuffInfo;

use super::BuffDefinition;
use crate::engine::skill::buff_act::registry::BuffActKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedBuffFeature {
    pub raw: String,
    pub values: Vec<i32>,
    pub act_type: String,
    pub effect_time: i32,
    pub effect_condition: i32,
    pub kind: Option<crate::engine::skill::buff_act::registry::BuffActKind>,
    pub arguments_supported: bool,
    pub stat_read_timing: crate::engine::skill::buff_act::registry::StatReadTiming,
    pub wire: Option<&'static crate::engine::skill::buff_act::wire::BuffActWireDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffPowerMaxAdd {
    pub buff_uid: i64,
    pub owner_uid: i64,
    pub power_id: i32,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffHpMaxAddRate {
    pub buff_uid: i64,
    pub owner_uid: i64,
    pub permille: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveBuffFeature {
    pub owner_uid: i64,
    pub source_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub amount: i32,
    pub team_type: i32,
    pub owner_alive: bool,
    pub act_type: String,
    pub effect_time: i32,
    pub effect_condition: i32,
    pub raw: String,
    pub values: Vec<i32>,
}

impl ActiveBuffFeature {
    pub fn act_id(&self) -> Option<i32> {
        self.values.first().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffPassiveSkillLink {
    pub owner_uid: i64,
    pub runtime_target_uid: i64,
    pub skill_id: i32,
}

pub(super) fn active_feature(
    owner_uid: i64,
    team_type: i32,
    owner_alive: bool,
    buff: &BuffInfo,
    definition: Option<&BuffDefinition>,
) -> Vec<ActiveBuffFeature> {
    let Some(definition) = definition else {
        return Vec::new();
    };
    let buff_id = buff.buff_id.unwrap_or_default();
    let mut visited = vec![buff_id];
    active_features_for_definition(
        owner_uid,
        team_type,
        owner_alive,
        buff,
        buff_id,
        definition,
        &mut visited,
    )
}

fn active_features_for_definition(
    owner_uid: i64,
    team_type: i32,
    owner_alive: bool,
    buff: &BuffInfo,
    feature_buff_id: i32,
    definition: &BuffDefinition,
    visited: &mut Vec<i32>,
) -> Vec<ActiveBuffFeature> {
    if definition.has_effect_count() && buff.count.unwrap_or_default() <= 0 {
        return Vec::new();
    }
    let mut output = Vec::new();
    for feature in definition.features() {
        output.push(ActiveBuffFeature {
            owner_uid,
            source_uid: buff.from_uid.unwrap_or_default(),
            buff_uid: buff.uid.unwrap_or_default(),
            buff_id: feature_buff_id,
            amount: super::count_or_layer(buff),
            team_type,
            owner_alive,
            act_type: feature.act_type.clone(),
            effect_time: feature.effect_time,
            effect_condition: feature.effect_condition,
            values: feature.values.clone(),
            raw: feature.raw.clone(),
        });
        if feature.kind != Some(crate::engine::skill::buff_act::registry::BuffActKind::SubBuff) {
            continue;
        }
        let Some(&child_buff_id) = feature.values.get(1) else {
            continue;
        };
        if visited.contains(&child_buff_id) {
            continue;
        }
        let Some(child) = BuffDefinition::get(child_buff_id) else {
            continue;
        };
        visited.push(child_buff_id);
        output.extend(active_features_for_definition(
            owner_uid,
            team_type,
            owner_alive,
            buff,
            child_buff_id,
            &child,
            visited,
        ));
        visited.pop();
    }
    output
}

pub(super) fn power_max_add(
    owner_uid: i64,
    buff: &BuffInfo,
    features: &[ResolvedBuffFeature],
    amount: i32,
) -> Vec<BuffPowerMaxAdd> {
    features
        .iter()
        .filter_map(move |feature| {
            if feature.kind != Some(BuffActKind::PowerMaxAdd) {
                return None;
            }
            let [_, power_id, delta] = feature.values.as_slice() else {
                return None;
            };
            (*power_id > 0 && *delta != 0).then_some(BuffPowerMaxAdd {
                buff_uid: buff.uid.unwrap_or_default(),
                owner_uid,
                power_id: *power_id,
                delta: *delta * amount,
            })
        })
        .collect()
}

pub(super) fn hp_max_add_rate(
    owner_uid: i64,
    buff: &BuffInfo,
    features: &[ResolvedBuffFeature],
    amount: i32,
) -> Vec<BuffHpMaxAddRate> {
    features
        .iter()
        .filter_map(move |feature| {
            if feature.kind != Some(BuffActKind::Attr) {
                return None;
            }
            let [_, 101, permille] = feature.values.as_slice() else {
                return None;
            };
            (*permille != 0).then_some(BuffHpMaxAddRate {
                buff_uid: buff.uid.unwrap_or_default(),
                owner_uid,
                permille: *permille * amount,
            })
        })
        .collect()
}

pub(super) fn passive_skill_links(
    owner_uid: i64,
    features: &[ResolvedBuffFeature],
    amount: i32,
) -> Vec<BuffPassiveSkillLink> {
    let mut output = Vec::new();
    collect_passive_skill_links(owner_uid, features, amount, &mut Vec::new(), &mut output);
    output
}

fn collect_passive_skill_links(
    owner_uid: i64,
    features: &[ResolvedBuffFeature],
    amount: i32,
    visited: &mut Vec<i32>,
    output: &mut Vec<BuffPassiveSkillLink>,
) {
    for feature in features {
        let skill_id = match (feature.kind, feature.values.as_slice()) {
            (Some(BuffActKind::AddPassiveSkills), [_, skill_id]) => Some(*skill_id),
            (Some(BuffActKind::AddPassiveSkillByLayer), [_, threshold, skill_id])
                if amount >= *threshold =>
            {
                Some(*skill_id)
            }
            _ => None,
        };
        if let Some(skill_id) = skill_id.filter(|skill_id| *skill_id > 0) {
            output.push(BuffPassiveSkillLink {
                owner_uid,
                runtime_target_uid: owner_uid,
                skill_id,
            });
        }
        if feature.kind != Some(crate::engine::skill::buff_act::registry::BuffActKind::SubBuff) {
            continue;
        }
        let Some(&child_buff_id) = feature.values.get(1) else {
            continue;
        };
        if visited.contains(&child_buff_id) {
            continue;
        }
        let Some(child) = BuffDefinition::get(child_buff_id) else {
            continue;
        };
        visited.push(child_buff_id);
        collect_passive_skill_links(owner_uid, child.features(), amount, visited, output);
        visited.pop();
    }
}

pub(super) fn resolve_features(raw_features: &str) -> Vec<ResolvedBuffFeature> {
    raw_features
        .split('|')
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            let values = raw
                .split(['#', ','])
                .map(str::trim)
                .filter_map(|part| part.parse().ok())
                .collect::<Vec<_>>();
            let act = values
                .first()
                .and_then(|act_id| config::try_get()?.buff_act.get(*act_id));
            let registered = act.and_then(|act| {
                crate::engine::skill::buff_act::registry::find(act.id, &act.r#type)
            });
            let arguments_supported = registered.is_some_and(|definition| {
                definition
                    .supports
                    .is_none_or(|supports| supports(values.get(1..).unwrap_or_default()))
            });
            ResolvedBuffFeature {
                raw: raw.to_owned(),
                values,
                act_type: act.map(|act| act.r#type.clone()).unwrap_or_default(),
                effect_time: act.map(|act| act.effect_time).unwrap_or_default(),
                effect_condition: act.map(|act| act.effect_condition).unwrap_or_default(),
                kind: registered.map(|definition| definition.kind),
                arguments_supported,
                stat_read_timing: registered
                    .map(|definition| definition.state.read_timing)
                    .unwrap_or(crate::engine::skill::buff_act::registry::StatReadTiming::None),
                wire: registered.and_then(|definition| definition.wire.as_ref()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_effect_count_buff_keeps_packet_state_but_stops_contributing() {
        crate::test_support::init_config();
        let buff = |count| BuffInfo {
            uid: Some(3),
            buff_id: Some(301),
            from_uid: Some(2),
            count: Some(count),
            ..Default::default()
        };

        let definition = BuffDefinition::get(301).unwrap();
        assert!(active_feature(1, 1, true, &buff(0), Some(&definition)).is_empty());
        assert_eq!(
            active_feature(1, 1, true, &buff(1), Some(&definition))[0].amount,
            1
        );
    }

    #[test]
    fn feature_lists_keep_each_configured_integer() {
        let feature = resolve_features("879#1#300#1,2").remove(0);

        assert_eq!(feature.values, vec![879, 1, 300, 1, 2]);
    }

    #[test]
    fn sub_buff_exposes_child_features_and_passive_links() {
        crate::test_support::init_config();
        let definition = BuffDefinition::get(31260151).unwrap();
        let buff = BuffInfo {
            uid: Some(7),
            buff_id: Some(31260151),
            from_uid: Some(10),
            layer: Some(1),
            count: Some(1),
            ..Default::default()
        };

        let features = active_feature(10, 1, true, &buff, Some(&definition));

        assert!(
            features
                .iter()
                .any(|feature| { feature.buff_id == 31260201 && feature.act_id() == Some(932) })
        );
        assert!(
            features
                .iter()
                .any(|feature| { feature.buff_id == 31260201 && feature.act_id() == Some(865) })
        );
        assert_eq!(
            passive_skill_links(10, definition.features(), 1),
            vec![BuffPassiveSkillLink {
                owner_uid: 10,
                runtime_target_uid: 10,
                skill_id: 31260181,
            }]
        );
    }
}
