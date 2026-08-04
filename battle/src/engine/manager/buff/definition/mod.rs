use std::{collections::HashMap, sync::OnceLock};

use crate::engine::entity::attr::AttrId;
use crate::engine::skill::buff_act::registry::BuffActKind;

use super::{BuffStatus, feature::ResolvedBuffFeature};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum BuffIncludeType {
    ReplaceExisting = 1,
    ExistingRefresh = 2,
    OwnUid = 3,
    SeparateTimedCopies = 4,
    PermanentMechanicCarrier = 5,
    SharedTypeFamily = 6,
    ReapplyReserve = 7,
    StateCarrier = 8,
    Count = 9,
    Stacked = 10,
    Layer = 11,
    Stacked12 = 12,
    GroupCapacity = 13,
    Stacked14 = 14,
    TimedIndependentStacks = 15,
    Counted = 16,
    CappedSeparateCopies = 17,
}

impl BuffIncludeType {
    const STACK_TYPES: [Self; 4] = [
        Self::Stacked,
        Self::Stacked12,
        Self::Stacked14,
        Self::TimedIndependentStacks,
    ];
    const LAYER_TYPES: [Self; 5] = [
        Self::Stacked,
        Self::Layer,
        Self::Stacked12,
        Self::Stacked14,
        Self::TimedIndependentStacks,
    ];

    fn id(self) -> i32 {
        self as i32
    }

    fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            1 => Self::ReplaceExisting,
            2 => Self::ExistingRefresh,
            3 => Self::OwnUid,
            4 => Self::SeparateTimedCopies,
            5 => Self::PermanentMechanicCarrier,
            6 => Self::SharedTypeFamily,
            7 => Self::ReapplyReserve,
            8 => Self::StateCarrier,
            9 => Self::Count,
            10 => Self::Stacked,
            11 => Self::Layer,
            12 => Self::Stacked12,
            13 => Self::GroupCapacity,
            14 => Self::Stacked14,
            15 => Self::TimedIndependentStacks,
            16 => Self::Counted,
            17 => Self::CappedSeparateCopies,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::ReplaceExisting => "ReplaceExisting",
            Self::ExistingRefresh => "ExistingRefresh",
            Self::OwnUid => "OwnUid",
            Self::SeparateTimedCopies => "SeparateTimedCopies",
            Self::PermanentMechanicCarrier => "PermanentMechanicCarrier",
            Self::SharedTypeFamily => "SharedTypeFamily",
            Self::ReapplyReserve => "ReapplyReserve",
            Self::StateCarrier => "ExclusiveTypeState",
            Self::Count => "Count",
            Self::Stacked => "Stacked",
            Self::Layer => "Layer",
            Self::Stacked12 => "Stacked12",
            Self::GroupCapacity => "GroupCapacity",
            Self::Stacked14 => "Stacked14",
            Self::TimedIndependentStacks => "TimedIndependentStacks",
            Self::Counted => "Counted",
            Self::CappedSeparateCopies => "CappedSeparateCopies",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BuffDefinition {
    id: i32,
    type_id: i32,
    pub(super) group: i32,
    is_no_show: bool,
    pub(super) status_id: i32,
    pub status: BuffStatus,
    pub duration: i32,
    count: i32,
    exclude_buff_ids: Vec<i32>,
    exclude_status_ids: Vec<i32>,
    include_entries: Vec<(i32, i32)>,
    include_types_valid: bool,
    attribute_deltas: Vec<(AttrId, i32)>,
    features: Vec<ResolvedBuffFeature>,
    has_features: bool,
    pub act_common_params: String,
    pub take_stage: i32,
    take_act: String,
}

impl BuffDefinition {
    pub(super) fn include_entry_name(include_type: i32, value: i32) -> String {
        let name = if include_type == BuffIncludeType::ReapplyReserve.id() && value > 0 {
            "ValueBearingType7"
        } else {
            BuffIncludeType::from_id(include_type)
                .map(BuffIncludeType::name)
                .unwrap_or("Unknown")
        };
        if value > 0 {
            format!("{name}({include_type}#{value})")
        } else {
            format!("{name}({include_type})")
        }
    }

    pub fn get(buff_id: i32) -> Option<Self> {
        static DEFINITIONS: OnceLock<HashMap<i32, BuffDefinition>> = OnceLock::new();
        let db = config::try_get()?;
        DEFINITIONS
            .get_or_init(|| {
                db.skill_buff
                    .all()
                    .iter()
                    .map(|row| {
                        let effective_type_id = if row.type_id != 0 {
                            row.type_id
                        } else {
                            row.id
                        };
                        let buff_type = db.skill_bufftype.get(effective_type_id);
                        let include_types = buff_type
                            .map(|row| row.include_types.as_str())
                            .unwrap_or_default();
                        let exclude_types = buff_type
                            .map(|row| row.exclude_types.as_str())
                            .unwrap_or_default();
                        let take_stage = buff_type.map(|row| row.take_stage).unwrap_or_default();
                        let features = row.features.as_str();
                        let status_id = buff_type.map(|row| row.r#type).unwrap_or(row.is_good_buff);
                        let include_entries = parse_include_entries(include_types);
                        let include_types_valid = include_entries.is_ok();
                        (
                            row.id,
                            Self {
                                id: row.id,
                                type_id: row.type_id,
                                group: buff_type.map(|row| row.group).unwrap_or_default(),
                                is_no_show: row.is_no_show != 0,
                                status_id,
                                status: BuffStatus::from_id(status_id),
                                duration: row.during_time,
                                count: row.effect_count,
                                exclude_buff_ids: parse_exclude_buff_ids(exclude_types),
                                exclude_status_ids: parse_exclude_status_ids(exclude_types),
                                include_entries: include_entries.unwrap_or_default(),
                                include_types_valid,
                                attribute_deltas: parse_attribute_deltas(features),
                                features: super::feature::resolve_features(features),
                                has_features: !features.trim().is_empty(),
                                act_common_params: initial_act_common_params(features),
                                take_stage,
                                take_act: buff_type
                                    .map(|row| row.take_act.clone())
                                    .unwrap_or_default(),
                            },
                        )
                    })
                    .collect()
            })
            .get(&buff_id)
            .cloned()
    }

    pub fn effective_type_id(&self) -> i32 {
        if self.type_id != 0 {
            self.type_id
        } else {
            self.id
        }
    }

    pub(super) fn id(&self) -> i32 {
        self.id
    }

    pub fn uses_stack_layer(&self) -> bool {
        self.is_stackable_type() || self.is_layer_type()
    }

    pub(super) fn accepts_explicit_grant_amount(&self) -> bool {
        self.uses_stack_layer()
            || (!self.uses_stack_layer()
                && self.count > 0
                && self.effective_type_id() == self.id
                && self.status_id == 5
                && self.has_include_type(BuffIncludeType::ExistingRefresh)
                && self.has_features
                && self
                    .features
                    .iter()
                    .any(|feature| feature.kind == Some(BuffActKind::Bullet))
                && self.take_act.is_empty())
    }

    pub fn exclude_buff_ids(&self) -> &[i32] {
        &self.exclude_buff_ids
    }

    pub fn exclude_status_ids(&self) -> &[i32] {
        &self.exclude_status_ids
    }

    pub(super) fn features(&self) -> &[ResolvedBuffFeature] {
        &self.features
    }

    pub(super) fn take_action(&self) -> Option<super::BuffTakeAction> {
        super::BuffTakeAction::parse(&self.take_act)
    }

    pub(super) fn stack_transition(&self) -> Option<(i32, i32)> {
        use crate::engine::skill::buff_act::registry::BuffActKind;

        self.features.iter().find_map(|feature| {
            (feature.kind == Some(BuffActKind::BuffReplace))
                .then(|| Some((*feature.values.get(1)?, *feature.values.get(2)?)))?
        })
    }

    pub(super) fn wire_markers(
        &self,
        phase: crate::engine::skill::buff_act::wire::WirePhase,
    ) -> Vec<i32> {
        self.features
            .iter()
            .filter(|feature| {
                phase == crate::engine::skill::buff_act::wire::WirePhase::Static
                    || !mutates_max_hp(feature)
            })
            .flat_map(|feature| {
                feature
                    .wire
                    .into_iter()
                    .flat_map(move |definition| definition.markers(phase).iter().copied())
            })
            .collect()
    }

    pub(super) fn state_snapshot_wire(&self, params: Option<&str>) -> Vec<(i32, Option<String>)> {
        self.features
            .iter()
            .filter_map(|feature| feature.wire)
            .flat_map(|wire| {
                wire.markers(crate::engine::skill::buff_act::wire::WirePhase::Refresh)
                    .iter()
                    .copied()
                    .map(move |effect_type| (effect_type, wire.snapshot_reserve_str(params)))
            })
            .collect()
    }

    pub(super) fn fanout_wire_markers(
        &self,
        phase: crate::engine::skill::buff_act::wire::WirePhase,
    ) -> Vec<i32> {
        use crate::engine::skill::buff_act::registry::BuffActKind;

        self.features
            .iter()
            .filter(|feature| {
                !matches!(
                    feature.kind,
                    Some(
                        BuffActKind::HaloBase
                            | BuffActKind::MasterHalo
                            | BuffActKind::LayerMasterHalo
                            | BuffActKind::SlaveHalo
                    )
                ) && !mutates_max_hp(feature)
            })
            .flat_map(|feature| {
                feature
                    .wire
                    .into_iter()
                    .flat_map(|definition| definition.markers(phase).iter().copied())
            })
            .collect()
    }

    pub(super) fn refreshes_unchanged(&self) -> bool {
        self.features
            .iter()
            .filter_map(|feature| feature.wire)
            .any(|wire| wire.refreshes_unchanged)
    }

    pub(super) fn pre_add_wire_effects(&self, target_uid: i64) -> Vec<super::BuffWireEffectResult> {
        self.features
            .iter()
            .filter_map(|feature| feature.wire?.pre_add)
            .map(|effect| super::BuffWireEffectResult {
                target_uid,
                effect_type: effect.effect_type,
                effect_num: effect.effect_num,
                effect_num1: effect.effect_num1,
            })
            .collect()
    }

    pub(super) fn initial_wire_states(
        &self,
        target_uid: i64,
        buff_uid: i64,
        team_type: i32,
        current_hp: i32,
    ) -> Vec<super::BuffActInfoMarkerResult> {
        use crate::engine::skill::buff_act::wire::InitialStateRule;

        self.features
            .iter()
            .filter_map(|feature| {
                let definition = feature.wire?;
                let act_id = feature.values.first().copied()?;
                let (params, str_param, marker_team) = match definition.initial_state? {
                    InitialStateRule::CrystalSelection => (
                        vec![
                            feature.values.get(1).copied().unwrap_or_default(),
                            feature.values.get(2).copied().unwrap_or_default(),
                            0,
                        ],
                        String::new(),
                        team_type,
                    ),
                    InitialStateRule::ConduitCardSelection => (
                        crate::engine::skill::buff_act::conduit_select::initial_params(
                            feature.values.get(1..)?,
                        )?,
                        String::new(),
                        team_type,
                    ),
                    InitialStateRule::ButterflyAllowedSkillKinds => (
                        feature.values.get(3..)?.to_vec(),
                        format!("{},0,0", feature.values.get(1)?),
                        team_type,
                    ),
                    InitialStateRule::HeatScale => (vec![0], String::new(), 0),
                    InitialStateRule::CurrentHpPermille => (
                        Vec::new(),
                        (current_hp * feature.values.get(1).copied().unwrap_or_default() / 1000)
                            .to_string(),
                        0,
                    ),
                    InitialStateRule::FirstArgument => (
                        vec![feature.values.get(1).copied().unwrap_or_default()],
                        String::new(),
                        0,
                    ),
                    InitialStateRule::SecondArgument => (
                        vec![feature.values.get(2).copied().unwrap_or_default()],
                        String::new(),
                        0,
                    ),
                    InitialStateRule::GrantValue => return None,
                };
                Some(super::BuffActInfoMarkerResult {
                    target_uid,
                    buff_uid,
                    act_id,
                    params,
                    str_param: Some(str_param),
                    team_type: marker_team,
                })
            })
            .collect()
    }

    pub(super) fn has_effect_count(&self) -> bool {
        self.count > 0
    }

    pub fn blocks_buff_id(&self, buff_id: i32) -> bool {
        self.exclude_buff_ids.contains(&buff_id)
    }

    pub fn blocks_status_id(&self, status_id: i32) -> bool {
        self.exclude_status_ids.contains(&status_id)
    }

    pub fn attribute_deltas(&self) -> &[(AttrId, i32)] {
        &self.attribute_deltas
    }

    pub(super) fn initial_grant_value_act_info(
        &self,
        grant_values: &[(i32, i32)],
    ) -> Option<Vec<sonettobuf::BuffActInfo>> {
        let values = self
            .features
            .iter()
            .filter(|feature| {
                feature.wire.and_then(|wire| wire.initial_state)
                    == Some(crate::engine::skill::buff_act::wire::InitialStateRule::GrantValue)
            })
            .filter_map(|feature| {
                let act_id = *feature.values.first()?;
                let value = grant_values
                    .iter()
                    .find_map(|(actual, value)| (*actual == act_id).then_some(*value))?;
                Some(sonettobuf::BuffActInfo {
                    act_id: Some(act_id),
                    param: vec![value],
                    str_param: Some(String::new()),
                })
            })
            .collect::<Vec<_>>();
        (!values.is_empty()).then_some(values)
    }

    pub fn count(&self, intent_count: i32) -> i32 {
        if intent_count > 0 {
            intent_count
        } else {
            self.count
        }
    }

    pub fn layer(&self, intent_layer: i32, layer_specified: bool, intent_count: i32) -> i32 {
        self.cap_layer(self.raw_layer(intent_layer, layer_specified, intent_count))
    }

    pub(super) fn raw_layer(
        &self,
        intent_layer: i32,
        layer_specified: bool,
        intent_count: i32,
    ) -> i32 {
        if self.uses_stack_layer() {
            if layer_specified {
                intent_layer
            } else if intent_count > 0 {
                intent_count
            } else if self.count > 0 {
                self.count
            } else {
                1
            }
        } else if self.uses_typed_count() {
            0
        } else if layer_specified {
            intent_layer
        } else {
            0
        }
    }

    pub fn cap_layer(&self, layer: i32) -> i32 {
        let max = BuffIncludeType::LAYER_TYPES
            .into_iter()
            .find_map(|kind| {
                self.include_entries.iter().find_map(|(actual, value)| {
                    (*actual == kind.id() && *value > 0).then_some(*value)
                })
            })
            .unwrap_or_default();
        if max > 0 { layer.min(max) } else { layer }
    }

    pub(super) fn stack_max_layer(&self) -> i32 {
        BuffIncludeType::LAYER_TYPES
            .into_iter()
            .find_map(|kind| {
                self.include_entries
                    .iter()
                    .find_map(|(actual, value)| (*actual == kind.id()).then_some(*value))
            })
            .unwrap_or_default()
    }

    pub(super) fn reserves_child_after_first_apply(&self) -> bool {
        !self.has_include_type(BuffIncludeType::OwnUid)
            && self.has_features
            && ((self.has_include_type(BuffIncludeType::Stacked)
                && !self.is_no_show
                && self.status == BuffStatus::Special)
                || (self.uses_stack_layer() && self.is_no_show && self.stack_max_layer() == 3))
    }

    pub(super) fn reserves_child_before_explicit_layer_apply(&self) -> bool {
        self.is_no_show
            && self.uses_child_uid()
            && self.stack_max_layer() == 16
            && self.has_features
    }

    pub(super) fn reserves_child_before_reapply(&self) -> bool {
        self.count > 0
            && self
                .features
                .iter()
                .any(|feature| feature.kind == Some(BuffActKind::AddToTarget))
    }

    pub(super) fn is_stackable_type(&self) -> bool {
        BuffIncludeType::STACK_TYPES
            .into_iter()
            .any(|include| self.has_include_type(include))
    }

    pub(super) fn emits_existing_layer_on_refresh(&self) -> bool {
        [BuffIncludeType::Layer, BuffIncludeType::Stacked12]
            .into_iter()
            .any(|kind| self.has_include_type(kind))
    }

    pub(super) fn reserves_child_on_layer_refresh(&self) -> bool {
        self.status != BuffStatus::Special || self.is_layer_type()
    }

    fn is_layer_type(&self) -> bool {
        self.has_include_type(BuffIncludeType::Layer)
    }

    pub(super) fn uses_typed_count(&self) -> bool {
        (self.has_include_type(BuffIncludeType::Counted)
            || (!self.uses_stack_layer()
                && !self.has_include_type(BuffIncludeType::ReplaceExisting)
                && self.count > 0
                && !self.reapplies_consumable_charge()))
            && self.count > 0
    }

    pub(super) fn reserves_normal_uid_on_count_refresh(&self) -> bool {
        self.has_include_type(BuffIncludeType::ExistingRefresh)
    }

    pub(super) fn reserves_child_uid_on_count_refresh(&self) -> bool {
        self.has_include_type(BuffIncludeType::Counted)
    }

    pub(super) fn uses_shared_type_family(&self) -> bool {
        self.has_include_type(BuffIncludeType::SharedTypeFamily)
    }

    pub(super) fn unresolved_include_entries(&self) -> Vec<(i32, i32)> {
        self.include_entries
            .iter()
            .filter_map(|(include_type, value)| {
                match *include_type {
                    1 | 2 | 3 | 4 | 6 | 10 | 11 | 12 | 14 | 15 | 16 | 17 => false,
                    kind if kind == BuffIncludeType::PermanentMechanicCarrier.id() => false,
                    kind if kind == BuffIncludeType::ReapplyReserve.id() => *value != 0,
                    kind if kind == BuffIncludeType::GroupCapacity.id() => {
                        self.shared_group_capacity().is_none()
                    }
                    kind if kind == BuffIncludeType::StateCarrier.id() => true,
                    kind if kind == BuffIncludeType::Count.id() => true,
                    _ => true,
                }
                .then_some((*include_type, *value))
            })
            .collect()
    }

    pub(super) fn normal_reservations_before_reapply(&self) -> i32 {
        if self.is_no_show
            && self.duration == 1
            && self.replaces_existing_copy()
            && self.features.iter().any(|feature| {
                feature.kind
                    == Some(crate::engine::skill::buff_act::registry::BuffActKind::EmitterNumChange)
            })
            && self
                .include_entries
                .contains(&(BuffIncludeType::ReapplyReserve.id(), 0))
        {
            2
        } else {
            0
        }
    }

    pub(super) fn normal_reservations_after_first_apply(&self) -> i32 {
        (self.duration > 0
            && self
                .features
                .iter()
                .any(|feature| feature.kind == Some(BuffActKind::FixedHurt)))
        .into()
    }

    pub(super) fn uses_child_uid(&self) -> bool {
        self.uses_calculation_child_uid()
            || (self.uses_stack_layer() && !self.has_include_type(BuffIncludeType::OwnUid))
    }

    fn uses_calculation_child_uid(&self) -> bool {
        self.is_no_show
            && self.features.iter().any(|feature| {
                matches!(
                    feature.kind,
                    Some(
                        crate::engine::skill::buff_act::registry::BuffActKind::AttrOnlyCalDamageReplaceAttrAdCreator
                            | crate::engine::skill::buff_act::registry::BuffActKind::LifeAttackFixRate
                    )
                )
            })
    }

    pub(super) fn include_types_valid(&self) -> bool {
        self.include_types_valid
    }

    pub(super) fn include_entries(&self) -> &[(i32, i32)] {
        &self.include_entries
    }

    pub(super) fn matches_type_category(&self, type_id: i32) -> bool {
        self.status_id == type_id
            || self
                .include_entries
                .iter()
                .any(|(include_type, _)| *include_type == type_id)
    }

    pub(super) fn syncs_single_stack_duration(&self) -> bool {
        self.act_common_params.is_empty()
            && self.has_features
            && self.has_attr_feature(AttrId::Penetration)
    }

    pub(super) fn reapplies_as_new(&self) -> bool {
        let timed_copy = self.has_include_type(BuffIncludeType::SeparateTimedCopies);
        self.shared_group_capacity().is_some()
            || self.capped_separate_copy_limit().is_some()
            || self.reapplies_consumable_charge()
            || (!self.uses_stack_layer()
                && !self.is_no_show
                && self.duration > 0
                && self.count == 0
                && (timed_copy || !self.has_include_type(BuffIncludeType::ReplaceExisting)))
    }

    fn reapplies_consumable_charge(&self) -> bool {
        self.is_no_show
            && self.status == BuffStatus::Equipment
            && self.count > 0
            && !self.take_act.is_empty()
            && !self.has_include_type(BuffIncludeType::ReplaceExisting)
    }

    pub(super) fn capped_separate_copy_limit(&self) -> Option<i32> {
        self.include_entries
            .iter()
            .find_map(|(include_type, value)| {
                (*include_type == BuffIncludeType::CappedSeparateCopies.id() && *value > 0)
                    .then_some(*value)
            })
    }

    pub(super) fn shared_group_capacity(&self) -> Option<(i32, i32)> {
        self.include_entries
            .iter()
            .find_map(|(include_type, value)| {
                (*include_type == BuffIncludeType::GroupCapacity.id()
                    && self.group > 0
                    && *value > 0)
                    .then_some((self.group, *value))
            })
    }

    pub(super) fn keeps_permanent_instance(&self) -> bool {
        self.duration == 0
            && ((self.is_no_show && !self.has_features)
                || self.features.iter().any(|feature| {
                    matches!(
                        feature.kind,
                        Some(
                            crate::engine::skill::buff_act::registry::BuffActKind::HaloBase
                                | crate::engine::skill::buff_act::registry::BuffActKind::MasterHalo
                                | crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo
                        )
                    )
                }))
    }

    pub(super) fn is_enhanced_passive_variant_of(&self, incoming: &Self) -> bool {
        self.id != incoming.id
            && self.duration == 0
            && incoming.duration == 0
            && self.effective_type_id() == incoming.effective_type_id()
            && self.features.len() > incoming.features.len()
            && incoming
                .features
                .iter()
                .all(|feature| self.features.contains(feature))
            && incoming.features.iter().any(|feature| {
                feature.kind == Some(BuffActKind::AddPassiveSkills)
                    && feature.values.get(1).is_some_and(|skill_id| {
                        self.features.iter().any(|resident| {
                            resident.kind == Some(BuffActKind::AddPassiveSkills)
                                && resident.values.get(1) == Some(skill_id)
                        })
                    })
            })
    }

    pub(super) fn replaces_existing_copy(&self) -> bool {
        !self.uses_stack_layer() && !self.uses_typed_count()
    }

    pub(super) fn cleans_up_at_round_start(&self) -> bool {
        self.duration == 1 && !self.has_features && self.count == 0 && self.replaces_existing_copy()
    }

    fn has_attr_feature(&self, attr_id: AttrId) -> bool {
        self.features.iter().any(|feature| {
            feature.kind == Some(BuffActKind::Attr)
                && matches!(feature.values.as_slice(), [_, value, ..] if *value == attr_id as i32)
        })
    }

    fn has_include_type(&self, include_type: BuffIncludeType) -> bool {
        self.include_entries
            .iter()
            .any(|(actual, _)| *actual == include_type.id())
    }
}

fn mutates_max_hp(feature: &super::feature::ResolvedBuffFeature) -> bool {
    use crate::engine::{entity::attr::AttrId, skill::buff_act::registry::BuffActKind};

    match feature.kind {
        Some(BuffActKind::Attr | BuffActKind::EachChangeAttr) => {
            feature
                .values
                .get(1)
                .and_then(|value| AttrId::from_raw(*value))
                == Some(AttrId::Hp)
        }
        _ => false,
    }
}

fn parse_exclude_buff_ids(raw: &str) -> Vec<i32> {
    parse_exclude_values(raw, "2")
}

fn parse_exclude_status_ids(raw: &str) -> Vec<i32> {
    parse_exclude_values(raw, "1")
}

fn parse_exclude_values(raw: &str, expected_prefix: &str) -> Vec<i32> {
    raw.split('|')
        .filter_map(|entry| entry.trim().split_once('#'))
        .filter(|(prefix, _)| prefix.trim() == expected_prefix)
        .flat_map(|(_, values)| values.split(|ch| ch == ',' || ch as u32 == 0xff0c))
        .filter_map(|token| token.trim().parse::<i32>().ok())
        .filter(|id| *id > 0)
        .collect()
}

fn parse_attribute_deltas(features: &str) -> Vec<(AttrId, i32)> {
    features
        .split('|')
        .filter_map(|feature| {
            let values = feature
                .split('#')
                .filter_map(|value| value.trim().parse().ok())
                .collect::<Vec<i32>>();
            match values.as_slice() {
                [act_id, attr_id, value]
                    if config::try_get()
                        .and_then(|db| db.buff_act.get(*act_id))
                        .and_then(|act| {
                            crate::engine::skill::buff_act::registry::kind(*act_id, &act.r#type)
                        })
                        == Some(BuffActKind::Attr) =>
                {
                    Some((AttrId::from_raw(*attr_id)?, *value))
                }
                [act_id, value]
                    if config::try_get()
                        .and_then(|db| db.buff_act.get(*act_id))
                        .and_then(|act| {
                            crate::engine::skill::buff_act::registry::kind(*act_id, &act.r#type)
                        })
                        == Some(
                            crate::engine::skill::buff_act::registry::BuffActKind::RealHarmSkillEffectFix,
                        ) =>
                {
                    Some((AttrId::GenesisDmgBonus, *value))
                }
                _ => None,
            }
        })
        .collect()
}

fn parse_include_entries(raw: &str) -> Result<Vec<(i32, i32)>, ()> {
    if raw.trim().is_empty() || raw.trim() == "0" {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    for entry in raw
        .split(['|', ',', ';', ' '])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let mut parts = entry.split('#').map(str::trim);
        let include_type = parts.next().ok_or(())?.parse::<i32>().map_err(|_| ())?;
        if include_type <= 0 {
            return Err(());
        }
        let value = match parts.next() {
            Some(raw_value) if has_include_value(include_type) => {
                raw_value.parse::<i32>().map_err(|_| ())?
            }
            Some(_) => return Err(()),
            None => 0,
        };
        if parts.next().is_some() {
            return Err(());
        }
        output.push((include_type, value));
    }
    Ok(output)
}

fn has_include_value(include_type: i32) -> bool {
    matches!(include_type, 7 | 10 | 11 | 12 | 13 | 14 | 15 | 17)
}

fn initial_act_common_params(features: &str) -> String {
    features
        .split('|')
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .filter_map(|raw| raw.split('#').next()?.trim().parse::<i32>().ok())
        .find_map(|act_id| {
            let act = config::try_get()?.buff_act.get(act_id)?;
            match crate::engine::skill::buff_act::registry::kind(act_id, &act.r#type)? {
                crate::engine::skill::buff_act::registry::BuffActKind::EzioBigSkill => {
                    Some(format!("{act_id}#1,0,0"))
                }
                crate::engine::skill::buff_act::registry::BuffActKind::InjuryBank
                | crate::engine::skill::buff_act::registry::BuffActKind::ExPointOverflowBank
                | crate::engine::skill::buff_act::registry::BuffActKind::AddAttrBySpecialCount
                | crate::engine::skill::buff_act::registry::BuffActKind::SpecialCountContinueChannelBuff => {
                    Some(format!("{act_id}#0"))
                }
                _ => None,
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod test;
