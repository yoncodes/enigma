use std::collections::{HashMap, HashSet, VecDeque};

use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam};

use crate::engine::buff::{halo, marker};
use crate::engine::entity::attr::AttrId;
use crate::engine::manager::hp::HpManager;

mod apply;
mod command;
mod definition;
mod feature;
mod grant_plan;
mod lifecycle;
mod query;
mod result;
mod rules;
mod state;
mod status;
mod uid;
mod uid_policy;
mod update;

pub use crate::engine::skill::rule::CommandOrigin;
pub(crate) use command::BuffPlan;
pub use command::{
    BuffAccumulateActValue, BuffAmount, BuffChangeDuration, BuffChanges, BuffChildUidReservation,
    BuffCommand, BuffCommandError, BuffConsume, BuffConvert, BuffDispel, BuffDurationAdvance,
    BuffGrant, BuffGrantChild, BuffGrantRelation, BuffGrantUidReservation, BuffLifecycleTransition,
    BuffRefreshWire, BuffRemove, BuffRemoveSelector, BuffReplace, BuffRoundStartCleanup,
    BuffRoundStartDurationSync, BuffSelector, BuffSetAmount, BuffSetState, BuffSpecialCount,
    BuffStateSnapshotWire, DepletedBuff, RelatedBuffGrant,
};
use definition::BuffDefinition;
pub use feature::BuffPassiveSkillLink;
pub use feature::{ActiveBuffFeature, BuffHpMaxAddRate, BuffPowerMaxAdd};
use feature::{active_feature, hp_max_add_rate, passive_skill_links, power_max_add};
pub(crate) use lifecycle::BuffTakeAction;
pub use query::SpecialCountOutput;
pub use result::{
    BuffActInfoMarkerResult, BuffActTriggerResult, BuffApplyResult, BuffDeleteReason,
    BuffFanoutResult, BuffFanoutUpdateResult, BuffMarkerResult, BuffRejectResult, BuffRemoveResult,
    BuffReplaceResult, BuffShieldRemoveResult, BuffSyncResult, BuffUpdateResult,
    BuffWireEffectResult,
};
pub use rules::{
    BuffExclusions, BuffLifetime, BuffPolicy, BuffPolicyError, BuffStorage, DuplicateGrant,
    ExistingBuffMatch, SharedGroupCapacity, UidAllocationPolicy,
};
pub use status::BuffStatus;
use uid::{ATTACKER_BUFF_UID_START, BuffUidAllocator, DEFENDER_BUFF_UID_START};

pub(crate) fn configured_status(buff_id: i32) -> Option<BuffStatus> {
    BuffDefinition::get(buff_id).map(|definition| definition.status)
}

pub(crate) fn wire_markers(
    buff_id: i32,
    phase: crate::engine::skill::buff_act::wire::WirePhase,
) -> Vec<i32> {
    BuffDefinition::get(buff_id)
        .map(|definition| definition.wire_markers(phase))
        .unwrap_or_default()
}

pub(crate) fn state_snapshot_wire(
    buff_id: i32,
    params: Option<&str>,
) -> Vec<(i32, Option<String>)> {
    BuffDefinition::get(buff_id)
        .map(|definition| definition.state_snapshot_wire(params))
        .unwrap_or_default()
}

pub(crate) fn refreshes_unchanged(buff_id: i32) -> bool {
    BuffDefinition::get(buff_id).is_some_and(|definition| definition.refreshes_unchanged())
}

#[derive(Debug, Clone)]
/// Owns active buff instances, storage policy, private act state, and buff UID allocation.
/// Callers submit `BuffCommand`s rather than choosing stacking, exclusion, or UID policy.
pub struct BuffManager {
    buffs: Vec<ActiveBuff>,
    entities: Vec<TrackedEntity>,
    team_types: HashMap<i64, i32>,
    added_by_owner: HashMap<(i64, i32), i32>,
    added_by_team: HashMap<(i32, i32), i32>,
    added_history: HashMap<i64, Vec<i32>>,
    act_states: HashMap<(i64, i32), state::DotState>,
    act_values: HashMap<(i64, i32), i32>,
    grant_values: HashMap<(i64, i32), i32>,
    reserved_grant_uids: HashMap<(i64, i32), VecDeque<i64>>,
    transaction: BuffTransactionState,
    shared_uid_lane: bool,
    attacker: BuffUidAllocator,
    defender: BuffUidAllocator,
}

impl Default for BuffManager {
    fn default() -> Self {
        Self {
            buffs: Vec::new(),
            entities: Vec::new(),
            team_types: HashMap::new(),
            added_by_owner: HashMap::new(),
            added_by_team: HashMap::new(),
            added_history: HashMap::new(),
            act_states: HashMap::new(),
            act_values: HashMap::new(),
            grant_values: HashMap::new(),
            reserved_grant_uids: HashMap::new(),
            transaction: BuffTransactionState::default(),
            shared_uid_lane: false,
            attacker: BuffUidAllocator::new(ATTACKER_BUFF_UID_START),
            defender: BuffUidAllocator::new(DEFENDER_BUFF_UID_START),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BuffTransactionState {
    depth: usize,
    progressed_stack_buff_ids: HashSet<i32>,
}

impl BuffManager {
    pub(crate) fn begin_transaction(&mut self) {
        if self.transaction.depth == 0 {
            self.transaction.progressed_stack_buff_ids.clear();
        }
        self.transaction.depth += 1;
    }

    pub(crate) fn end_transaction(&mut self) {
        debug_assert!(self.transaction.depth > 0);
        self.transaction.depth = self.transaction.depth.saturating_sub(1);
        if self.transaction.depth == 0 {
            self.transaction.progressed_stack_buff_ids.clear();
        }
    }

    fn transaction_has_stack_progress(&self, buff_id: i32) -> bool {
        self.transaction.depth > 0
            && self
                .transaction
                .progressed_stack_buff_ids
                .contains(&buff_id)
    }

    fn record_transaction_stack_progress(&mut self, buff_id: i32) {
        if self.transaction.depth > 0 {
            self.transaction.progressed_stack_buff_ids.insert(buff_id);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveBuff {
    owner_uid: i64,
    team_type: i32,
    type_id: i32,
    definition: Option<BuffDefinition>,
    buff: BuffInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrackedEntity {
    uid: i64,
    team_type: i32,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuffAddArgs {
    layer: i32,
    count: i32,
    layer_specified: bool,
}

#[derive(Debug, Clone, Copy)]
struct BuffRoute {
    source_uid: i64,
    target_uid: i64,
    buff_id: i32,
}

impl BuffRoute {
    fn new(source_uid: i64, target_uid: i64, buff_id: i32) -> Self {
        Self {
            source_uid,
            target_uid,
            buff_id,
        }
    }
}

impl BuffAddArgs {
    fn layer(layer: i32) -> Self {
        Self {
            layer,
            count: 0,
            layer_specified: true,
        }
    }

    fn count(count: i32) -> Self {
        Self {
            layer: 0,
            count,
            layer_specified: false,
        }
    }
}

fn attribute_amount(buff: &BuffInfo) -> i32 {
    buff.layer
        .filter(|layer| *layer > 0)
        .or_else(|| buff.count.filter(|count| *count > 0))
        .unwrap_or(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum LayerHaloWireType {
    Master = 1,
    Slave = 2,
}

fn buff_wire_type(buff_id: i32, source_uid: i64, target_uid: i64) -> i32 {
    if halo::has_layer_master(buff_id) {
        if source_uid == target_uid {
            LayerHaloWireType::Master as i32
        } else {
            LayerHaloWireType::Slave as i32
        }
    } else {
        0
    }
}

fn is_layer_halo_slave(buff: &BuffInfo) -> bool {
    buff.r#type == Some(LayerHaloWireType::Slave as i32)
}

fn fallback_type_id(buff: &BuffInfo) -> i32 {
    buff.buff_id
        .filter(|id| *id != 0)
        .or(buff.r#type.filter(|id| *id != 0))
        .unwrap_or_default()
}

fn count_or_layer(buff: &BuffInfo) -> i32 {
    match buff.buff_id.and_then(BuffDefinition::get) {
        Some(definition) if definition.uses_stack_layer() => buff.layer.unwrap_or_default().max(0),
        Some(definition) if definition.uses_typed_count() => buff.count.unwrap_or_default().max(0),
        _ => buff
            .count
            .filter(|count| *count > 0)
            .or_else(|| buff.layer.filter(|layer| *layer > 0))
            .unwrap_or(1),
    }
}

impl BuffManager {
    fn reserve_grant_uid(&mut self, target_uid: i64, buff_id: i32, uid: i64) {
        self.reserved_grant_uids
            .entry((target_uid, buff_id))
            .or_default()
            .push_back(uid);
    }

    fn reserved_grant_uid(&self, target_uid: i64, buff_id: i32) -> Option<i64> {
        self.reserved_grant_uids
            .get(&(target_uid, buff_id))?
            .front()
            .copied()
    }

    fn take_reserved_grant_uid(&mut self, target_uid: i64, buff_id: i32) -> Option<i64> {
        let key = (target_uid, buff_id);
        let queue = self.reserved_grant_uids.get_mut(&key)?;
        let uid = queue.pop_front();
        let empty = queue.is_empty();
        if empty {
            self.reserved_grant_uids.remove(&key);
        }
        uid
    }

    fn record_added(&mut self, owner_uid: i64, buff_id: i32, amount: i32) {
        if amount <= 0 {
            return;
        }
        *self.added_by_owner.entry((owner_uid, buff_id)).or_default() += amount;
        if let Some(team_type) = self.team_type(owner_uid) {
            *self.added_by_team.entry((team_type, buff_id)).or_default() += amount;
        }
        self.added_history
            .entry(owner_uid)
            .or_default()
            .push(buff_id);
    }

    pub fn added_history_for_owner(&self, owner_uid: i64) -> &[i32] {
        self.added_history
            .get(&owner_uid)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn added_count_for_owner(&self, owner_uid: i64, buff_ids: &[i32]) -> i32 {
        buff_ids
            .iter()
            .map(|buff_id| {
                self.added_by_owner
                    .get(&(owner_uid, *buff_id))
                    .copied()
                    .unwrap_or(0)
            })
            .sum()
    }

    pub fn added_count_for_team(&self, team_type: i32, buff_ids: &[i32]) -> i32 {
        buff_ids
            .iter()
            .map(|buff_id| {
                self.added_by_team
                    .get(&(team_type, *buff_id))
                    .copied()
                    .unwrap_or(0)
            })
            .sum()
    }
}

fn typed_count_repeat(
    definition: &BuffDefinition,
    layer: i32,
    layer_specified: bool,
    count: i32,
) -> i32 {
    if !definition.uses_typed_count() {
        1
    } else if count > 0 {
        count
    } else if layer_specified && layer > 0 {
        layer
    } else {
        1
    }
}

pub(crate) fn emits_existing_layer_on_refresh(buff_id: i32) -> bool {
    BuffDefinition::get(buff_id)
        .is_some_and(|definition| definition.emits_existing_layer_on_refresh())
}

#[cfg(test)]
mod tests;
