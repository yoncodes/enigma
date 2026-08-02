use sonettobuf::BuffActInfo;

use crate::engine::{
    event::payload::BattleEvent,
    manager::hp::HpManager,
    skill::rule::{CommandOrigin, RuleDomain},
};

use super::{
    ActiveBuff, BuffActInfoMarkerResult, BuffAddArgs, BuffDefinition, BuffDeleteReason,
    BuffManager, BuffMarkerResult, BuffPolicy, BuffReplaceResult, BuffRoute,
    BuffShieldRemoveResult, BuffStatus, count_or_layer,
    grant_plan::{GrantAction, LayerRefreshPlan, PlannedFanout, PlannedFanoutRefresh},
    typed_count_repeat,
    uid_policy::{self, UidAllocationPlan},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffGrant {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub amount: Option<i32>,
    pub occurrences: u32,
    pub child_uid_reservations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffGrantRelation {
    Child,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelatedBuffGrant {
    pub grant: BuffGrant,
    pub relation: BuffGrantRelation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffGrantChild {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub amount: Option<i32>,
    pub params: Option<String>,
    pub act_info: Option<Vec<BuffActInfo>>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GrantRequest {
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub input: GrantInput,
    pub occurrences: u32,
    pub child_uid_reservations: u32,
    pub force_normal_uid: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GrantInput {
    Default,
    Layer(i32),
    ChildUid { layer: i32, layer_specified: bool },
    IndependentInstance { layer: i32, layer_specified: bool },
    ChildInstance { layer: i32, layer_specified: bool },
    TriggeredChildInstance { layer: i32, layer_specified: bool },
    UnconditionalLayer(i32),
}

impl From<&BuffGrant> for GrantRequest {
    fn from(grant: &BuffGrant) -> Self {
        Self {
            source_uid: grant.source_uid,
            target_uid: grant.target_uid,
            buff_id: grant.buff_id,
            input: grant.amount.map_or(GrantInput::Default, GrantInput::Layer),
            occurrences: grant.occurrences,
            child_uid_reservations: grant.child_uid_reservations,
            force_normal_uid: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffSelector {
    IdOrType(i32),
    ExactId(i32),
    TypeId(i32),
    Uid(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepletedBuff {
    Keep,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffConsume {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub selector: BuffSelector,
    pub amount: i32,
    pub depleted: DepletedBuff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffRemoveSelector {
    Uid(i64),
    ExactId(i32),
    IdOrType(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffRemove {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub selector: BuffRemoveSelector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffDispel {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub statuses: Vec<super::BuffStatus>,
    pub excluded_ids_or_types: Vec<i32>,
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffReplace {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub source: BuffSelector,
    pub replacement_id_or_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffConvert {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub source_buff_id: i32,
    pub output_buff_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffAmount {
    Layer(i32),
    Count(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffSetAmount {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub amount: BuffAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffSetState {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub ex_info: Option<i32>,
    pub params: Option<String>,
    pub act_info: Option<Vec<BuffActInfo>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffAccumulateActValue {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub act_id: i32,
    pub delta: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffChangeDuration {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub selector: BuffSelector,
    pub delta: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffSpecialCount {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub marker_ids: Vec<i32>,
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffChildUidReservation {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffGrantUidReservation {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub buff_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffDurationAdvance {
    pub origin: CommandOrigin,
    pub take_stage: i32,
    pub owner_uids: Vec<i64>,
    pub buff_uids: Option<Vec<i64>>,
}

const ROUND_START_DURATION_SYNC_KEY: crate::engine::skill::rule::DefinitionKey =
    crate::engine::skill::rule::DefinitionKey::new(0, "RoundStartDurationSync");
const ROUND_START_CLEANUP_KEY: crate::engine::skill::rule::DefinitionKey =
    crate::engine::skill::rule::DefinitionKey::new(0, "RoundStartCleanup");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffRoundStartDurationSync {
    pub origin: CommandOrigin,
    pub owner_uids: Vec<i64>,
}

impl BuffRoundStartDurationSync {
    pub fn new(owner_uids: Vec<i64>) -> Self {
        Self {
            origin: CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: ROUND_START_DURATION_SYNC_KEY,
            },
            owner_uids,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffRoundStartCleanup {
    pub origin: CommandOrigin,
}

impl BuffRoundStartCleanup {
    pub fn new() -> Self {
        Self {
            origin: CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: ROUND_START_CLEANUP_KEY,
            },
        }
    }
}

impl Default for BuffRoundStartCleanup {
    fn default() -> Self {
        Self::new()
    }
}

impl BuffDurationAdvance {
    pub fn new(take_stage: i32, owner_uids: Vec<i64>, buff_uids: Option<Vec<i64>>) -> Option<Self> {
        let definition = crate::engine::skill::buff_act::effect_time::find(take_stage)?;
        Some(Self {
            origin: CommandOrigin {
                domain: RuleDomain::EffectTime,
                key: definition.key,
            },
            take_stage,
            owner_uids,
            buff_uids,
        })
    }

    pub fn for_event(event: &BattleEvent) -> Option<Self> {
        let BattleEvent::SkillAction(action) = event else {
            return None;
        };
        let take_stage =
            crate::engine::skill::buff_act::effect_time::duration_stage_for_skill_phase(
                action.phase,
            )?;
        Self::new(take_stage, vec![action.source_uid], None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuffCommand {
    Grant(BuffGrant),
    GrantRelated(RelatedBuffGrant),
    GrantIndependent(BuffGrant),
    Accumulate(BuffGrant),
    GrantStateful(BuffGrantChild),
    GrantUsingChildUid(BuffGrant),
    GrantUsingNormalUid(BuffGrant),
    GrantChild(BuffGrantChild),
    GrantInternalChild(BuffGrantChild),
    Consume(BuffConsume),
    ConsumeCount(BuffConsume),
    ConsumeEffectCount(BuffConsume),
    ConsumeCoalesced(BuffConsume),
    Convert(BuffConvert),
    Replace(BuffReplace),
    Remove(BuffRemove),
    RemoveAfterTrigger(BuffRemove),
    Deactivate(BuffRemove),
    ExpireAction(BuffRemove),
    Dispel(BuffDispel),
    SetAmount(BuffSetAmount),

    SetState(BuffSetState),
    SetInternalState(BuffSetState),
    SetStateSnapshot(BuffSetState),
    AccumulateActValue(BuffAccumulateActValue),
    ChangeDuration(BuffChangeDuration),
    AddSpecialCount(BuffSpecialCount),
    ReserveChildUids(BuffChildUidReservation),
    ReserveGrantUid(BuffGrantUidReservation),
    AdvanceDuration(BuffDurationAdvance),
    SyncRoundStartDuration(BuffRoundStartDurationSync),
    CleanupRoundStart(BuffRoundStartCleanup),
}

#[derive(Debug, Clone)]
pub(crate) struct BuffPlan {
    pub origin: CommandOrigin,
    action: BuffPlanAction,
}

impl BuffPlan {
    pub(crate) fn added_buff_uid(&self) -> Option<i64> {
        let plan = match &self.action {
            BuffPlanAction::Grant(plan) => plan.as_ref(),
            _ => return None,
        };
        matches!(plan.action, GrantAction::Add | GrantAction::ReplaceExisting)
            .then(|| plan.uid.map(|uid| uid.uid))
            .flatten()
    }

    pub(crate) fn initialize_added_act_value(
        &mut self,
        act_id: i32,
        value: i32,
        team_type: i32,
        current_hp: i32,
    ) -> Option<i64> {
        let uid = self.added_buff_uid()?;
        let plan = match &mut self.action {
            BuffPlanAction::Grant(plan) => plan.as_mut(),
            _ => unreachable!("added buff uid requires a grant plan"),
        };
        let act_info = plan.initial_act_info.get_or_insert_with(|| {
            plan.definition
                .initial_wire_states(plan.route.target_uid, uid, team_type, current_hp)
                .into_iter()
                .map(|marker| BuffActInfo {
                    act_id: Some(marker.act_id),
                    param: marker.params,
                    str_param: marker.str_param.or_else(|| Some(String::new())),
                })
                .collect()
        });
        if let Some(info) = act_info.iter_mut().find(|info| info.act_id == Some(act_id)) {
            info.param = vec![value];
            info.str_param = Some(String::new());
        } else {
            act_info.push(BuffActInfo {
                act_id: Some(act_id),
                param: vec![value],
                str_param: Some(String::new()),
            });
        }
        plan.initial_act_info_markers = Some(Vec::new());
        let order = plan
            .definition
            .features()
            .iter()
            .filter_map(|feature| feature.values.first().copied())
            .collect::<Vec<_>>();
        act_info.sort_by_key(|info| {
            order
                .iter()
                .position(|configured| Some(*configured) == info.act_id)
                .unwrap_or(usize::MAX)
        });
        Some(uid)
    }
}

#[derive(Debug, Clone)]
enum BuffPlanAction {
    Grant(Box<GrantPlan>),
    GrantInternalChild(Box<GrantPlan>),
    Accumulate(Box<GrantPlan>),
    Consume(ConsumePlan),
    ConsumeEffectCount(ConsumePlan),
    Convert(Box<ConvertPlan>),
    Replace(Box<ReplacePlan>),
    Remove(RemovePlan),
    SetAmount(SetAmountPlan),
    SetState(SetStatePlan),
    SetInternalState(SetStatePlan),
    SetStateSnapshot(SetStatePlan),
    AccumulateActValue(BuffAccumulateActValue),
    ChangeDuration(Vec<DurationChangePlan>),
    AddSpecialCount(SpecialCountPlan),
    ReserveChildUids(UidReservationPlan),
    ReserveGrantUid(GrantUidReservationPlan),
    AdvanceDuration(Vec<super::lifecycle::BuffDurationPlan>),
    SyncRoundStartDuration(Vec<super::lifecycle::BuffLifecyclePlan>),
    CleanupRoundStart(Vec<super::lifecycle::BuffLifecyclePlan>),
}

#[derive(Debug, Clone)]
struct ConsumePlan {
    target_uid: i64,
    actions: Vec<ConsumeAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConsumeField {
    Configured,
    Count,
}

#[derive(Debug, Clone)]
struct ConvertPlan {
    consume: ConsumePlan,
    grant: Option<GrantPlan>,
}

#[derive(Debug, Clone)]
struct ReplacePlan {
    target_uid: i64,
    removed_uids: Vec<i64>,
    grant: GrantPlan,
}

#[derive(Debug, Clone)]
struct RemovePlan {
    target_uid: i64,
    buff_uids: Vec<i64>,
    config_effect: i32,
    clear_amount: bool,
    clear_count: bool,
}

#[derive(Debug, Clone)]
struct SetAmountPlan {
    target_uid: i64,
    buff_uid: i64,
    amount: BuffAmount,
    exists: bool,
}

#[derive(Debug, Clone)]
struct SetStatePlan {
    target_uid: i64,
    buff_uid: i64,
    ex_info: Option<i32>,
    params: Option<String>,
    act_info: Option<Vec<BuffActInfo>>,
    exists: bool,
}

#[derive(Debug, Clone, Copy)]
struct DurationChangePlan {
    target_uid: i64,
    buff_uid: i64,
    duration: i32,
}

#[derive(Debug, Clone)]
struct SpecialCountPlan {
    target_uid: i64,
    marker_ids: Vec<i32>,
    count: i32,
}

#[derive(Debug, Clone)]
struct UidReservationPlan {
    target_uid: i64,
    uids: Vec<UidAllocationPlan>,
}

#[derive(Debug, Clone, Copy)]
struct GrantUidReservationPlan {
    target_uid: i64,
    buff_id: i32,
    uid: UidAllocationPlan,
}

#[derive(Debug, Clone)]
pub(super) enum ConsumeAction {
    Noop,
    SetInternalActInfo {
        buff_uid: i64,
        act_info: Vec<BuffActInfo>,
    },
    Update {
        buff_uid: i64,
        layer: Option<i32>,
        count: Option<i32>,
    },
    Remove {
        buff_uid: i64,
        layer: Option<i32>,
        count: Option<i32>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct GrantPlan {
    route: BuffRoute,
    definition: BuffDefinition,
    args: BuffAddArgs,
    repeat: i32,
    action: GrantAction,
    excluded_uids: Vec<i64>,
    replacement_uids: Vec<i64>,
    capacity_eviction_uids: Vec<i64>,
    pre_add_uids: Vec<UidAllocationPlan>,
    uid: Option<UidAllocationPlan>,
    refresh_uids: Vec<UidAllocationPlan>,
    layer_refresh_uid: Option<UidAllocationPlan>,
    layer_refresh: Option<LayerRefreshPlan>,
    fanout: Vec<PlannedFanout>,
    fanout_refreshes: Vec<PlannedFanoutRefresh>,
    post_add_uids: Vec<UidAllocationPlan>,
    initial_params: Option<String>,
    initial_act_info: Option<Vec<BuffActInfo>>,
    initial_act_info_markers: Option<Vec<BuffActInfo>>,
    dot_snapshots: Vec<super::state::DotSnapshotPlan>,
    grant_values: Vec<(i32, i32)>,
    immunity_action: Option<(i64, ConsumeAction)>,
    transition: Option<Box<ReplacePlan>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffCommandError {
    InvalidGrant,
    InvalidConsume,
    InvalidReplace,
    InvalidRemove,
    InvalidSetAmount,
    InvalidSetState,
    InvalidDurationChange,
    InvalidSpecialCount,
    InvalidUidReservation,
    InvalidDurationAdvance,
    InvalidLifecycle,
    MissingDefinition(i32),
    InvalidPolicy(super::BuffPolicyError),
    UnsupportedOccurrences(u32),
}

mod commit;
mod plan;
mod result;

pub use result::{BuffChanges, BuffLifecycleTransition, BuffRefreshWire, BuffStateSnapshotWire};

#[cfg(test)]
mod tests;
