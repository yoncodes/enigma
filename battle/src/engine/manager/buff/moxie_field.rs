use crate::engine::skill::buff_act::registry::BuffActKind;

use super::{ActiveBuff, BuffActInfoMarkerResult, BuffDefinition, BuffInfo, BuffManager};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MoxieField {
    act_id: i32,
    pub(crate) credited: i32,
    pub(crate) sampled: i32,
}

impl MoxieField {
    pub(super) fn from_buff(buff: &BuffInfo, definition: Option<&BuffDefinition>) -> Option<Self> {
        let feature = definition?.features().iter().find(|feature| {
            feature.kind == Some(BuffActKind::MoxieMightField) && feature.arguments_supported
        })?;
        let act_id = *feature.values.first()?;
        let [count] = buff
            .act_info
            .iter()
            .find(|state| state.act_id == Some(act_id))?
            .param
            .as_slice()
        else {
            return None;
        };
        Some(Self {
            act_id,
            credited: *count,
            sampled: *count,
        })
    }

    pub(super) fn observe(&mut self, observed: Self) {
        self.credited = if self.credited > self.sampled {
            self.credited.max(observed.sampled)
        } else {
            observed.sampled
        };
        self.sampled = observed.sampled;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Credit {
    pub(crate) owner_uid: i64,
    pub(crate) buff_uid: i64,
    pub(crate) amount: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub(crate) owner_uid: i64,
    pub(crate) buff_uid: i64,
}

impl BuffManager {
    pub(crate) fn moxie_field(&self, owner_uid: i64, buff_uid: i64) -> Option<MoxieField> {
        self.buffs
            .iter()
            .find(|buff| matches_field(buff, owner_uid, buff_uid))?
            .moxie_field
    }

    pub(crate) fn credit_moxie_field(&mut self, credit: Credit) {
        if let Some(field) = self
            .buffs
            .iter_mut()
            .find(|buff| matches_field(buff, credit.owner_uid, credit.buff_uid))
            .and_then(|buff| buff.moxie_field.as_mut())
        {
            field.credited = credit.amount;
        }
    }

    pub(crate) fn sample_moxie_field(&mut self, sample: Sample) -> Option<BuffActInfoMarkerResult> {
        let buff = self
            .buffs
            .iter_mut()
            .find(|buff| matches_field(buff, sample.owner_uid, sample.buff_uid))?;
        let field = buff
            .moxie_field
            .as_mut()
            .filter(|field| field.credited > 0)?;
        field.sampled = field.credited;
        if let Some(act_info) = buff
            .buff
            .act_info
            .iter_mut()
            .find(|state| state.act_id == Some(field.act_id))
        {
            act_info.param = vec![field.sampled];
        } else {
            buff.buff.act_info.push(sonettobuf::BuffActInfo {
                act_id: Some(field.act_id),
                param: vec![field.sampled],
                str_param: Some(String::new()),
            });
        }
        Some(BuffActInfoMarkerResult {
            target_uid: sample.owner_uid,
            buff_uid: sample.buff_uid,
            act_id: field.act_id,
            params: vec![field.sampled],
            str_param: Some(String::new()),
            team_type: 0,
        })
    }
}

fn matches_field(buff: &ActiveBuff, owner_uid: i64, buff_uid: i64) -> bool {
    buff.effective && buff.owner_uid == owner_uid && buff.buff.uid == Some(buff_uid)
}
