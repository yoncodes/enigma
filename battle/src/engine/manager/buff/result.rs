use sonettobuf::BuffInfo;

use crate::engine::{
    event::payload::{BattleEvent, BuffChangeEvent, BuffRejectedEvent, BuffStateChangeEvent},
    skill::rule::DefinitionKey,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BuffApplyResult {
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff: BuffInfo,
    pub pre_markers: Vec<BuffActInfoMarkerResult>,
    pub pre_effects: Vec<BuffWireEffectResult>,
    pub markers: Vec<BuffMarkerResult>,
    pub fanout: Vec<BuffApplyResult>,
    pub derived_by: Option<DefinitionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffWireEffectResult {
    pub target_uid: i64,
    pub effect_type: i32,
    pub effect_num: i32,
    pub effect_num1: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffSyncResult {
    pub target_uid: i64,
    pub buff: BuffInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffFanoutResult {
    pub rule: DefinitionKey,
    pub emitter_uid: i64,
    pub carrier_buff_uid: i64,
    pub carrier_buff_id: i32,
    pub added: Vec<BuffApplyResult>,
    pub refreshed: Vec<BuffFanoutUpdateResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffFanoutUpdateResult {
    pub update: BuffUpdateResult,
    pub markers: Vec<BuffMarkerResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffActInfoMarkerResult {
    pub target_uid: i64,
    pub buff_uid: i64,
    pub act_id: i32,
    pub params: Vec<i32>,
    pub str_param: Option<String>,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffMarkerResult {
    pub target_uid: i64,
    pub effect_type: i32,
    pub effect_num: i32,
    pub buff_act_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffActTriggerResult {
    pub target_uid: i64,
    pub buff_id: i32,
    pub buff_act_id: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffUpdateResult {
    pub target_uid: i64,
    pub before: BuffInfo,
    pub after: BuffInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffRemoveResult {
    pub target_uid: i64,
    pub before_amount: i32,
    pub buff: BuffInfo,
    pub config_effect: i32,
    pub delete_reason: Option<BuffDeleteReason>,
    pub depleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BuffDeleteReason {
    Overflow = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffShieldRemoveResult {
    pub target_uid: i64,
    pub buff_uid: i64,
    pub value: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuffReplaceResult {
    pub removed: Vec<BuffRemoveResult>,
    pub added: Option<BuffApplyResult>,
    pub refreshed: Vec<BuffUpdateResult>,
    pub rejected: Option<BuffRejectResult>,
    pub fanout: Vec<BuffFanoutResult>,
}

impl BuffReplaceResult {
    pub fn events(&self) -> Vec<BattleEvent> {
        fn push_added(events: &mut Vec<BattleEvent>, applied: &BuffApplyResult) {
            let buff = &applied.buff;
            events.push(BattleEvent::BuffAdded(BuffChangeEvent {
                source_uid: applied.source_uid,
                target_uid: applied.target_uid,
                buff_uid: buff.uid.unwrap_or_default(),
                buff_id: buff.buff_id.unwrap_or_default(),
                before_amount: 0,
                after_amount: super::count_or_layer(buff),
                act_id: buff
                    .act_info
                    .first()
                    .and_then(|info| info.act_id)
                    .unwrap_or_default(),
                act_value: buff
                    .act_info
                    .first()
                    .and_then(|info| info.param.first())
                    .copied()
                    .unwrap_or_default(),
            }));
            for fanout in &applied.fanout {
                push_added(events, fanout);
            }
        }

        let mut events = self
            .removed
            .iter()
            .map(|removed| {
                BattleEvent::BuffRemoved(BuffChangeEvent {
                    source_uid: removed.buff.from_uid.unwrap_or_default(),
                    target_uid: removed.target_uid,
                    buff_uid: removed.buff.uid.unwrap_or_default(),
                    buff_id: removed.buff.buff_id.unwrap_or_default(),
                    before_amount: removed.before_amount,
                    after_amount: 0,
                    act_id: removed
                        .buff
                        .act_info
                        .first()
                        .and_then(|info| info.act_id)
                        .unwrap_or_default(),
                    act_value: removed
                        .buff
                        .act_info
                        .first()
                        .and_then(|info| info.param.first())
                        .copied()
                        .unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        if let Some(added) = &self.added {
            push_added(&mut events, added);
        }
        events.extend(self.refreshed.iter().filter_map(|refresh| {
            let before_amount = super::count_or_layer(&refresh.before);
            let after_amount = super::count_or_layer(&refresh.after);
            (before_amount != after_amount).then_some(BattleEvent::BuffChanged(BuffChangeEvent {
                source_uid: refresh.after.from_uid.unwrap_or_default(),
                target_uid: refresh.target_uid,
                buff_uid: refresh.after.uid.unwrap_or_default(),
                buff_id: refresh.after.buff_id.unwrap_or_default(),
                before_amount,
                after_amount,
                act_id: refresh
                    .after
                    .act_info
                    .first()
                    .and_then(|info| info.act_id)
                    .unwrap_or_default(),
                act_value: refresh
                    .after
                    .act_info
                    .first()
                    .and_then(|info| info.param.first())
                    .copied()
                    .unwrap_or_default(),
            }))
        }));
        events.extend(self.refreshed.iter().filter_map(|refresh| {
            let before_ex_info = refresh.before.ex_info.unwrap_or_default();
            let after_ex_info = refresh.after.ex_info.unwrap_or_default();
            (before_ex_info != after_ex_info).then_some(BattleEvent::BuffStateChanged(
                BuffStateChangeEvent {
                    source_uid: refresh.after.from_uid.unwrap_or_default(),
                    target_uid: refresh.target_uid,
                    buff_uid: refresh.after.uid.unwrap_or_default(),
                    buff_id: refresh.after.buff_id.unwrap_or_default(),
                    before_ex_info,
                    after_ex_info,
                },
            ))
        }));
        if let Some(rejected) = &self.rejected {
            events.push(BattleEvent::BuffRejected(BuffRejectedEvent {
                source_uid: rejected.buff.from_uid.unwrap_or_default(),
                target_uid: rejected.target_uid,
                buff_uid: rejected.buff.uid.unwrap_or_default(),
                buff_id: rejected.buff.buff_id.unwrap_or_default(),
                type_id: rejected.type_id,
                blocker_buff_id: rejected.blocker_buff_id,
            }));
        }
        events
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffRejectResult {
    pub target_uid: i64,
    pub blocker_buff_id: i32,
    pub type_id: i32,
    pub buff: BuffInfo,
}
