use super::*;

impl BuffManager {
    #[cfg(test)]
    pub(crate) fn set_duration_by_id(
        &mut self,
        target_uid: i64,
        buff_id: i32,
        duration: i32,
    ) -> BuffReplaceResult {
        let mut refreshed = Vec::new();
        for active in self
            .buffs
            .iter_mut()
            .filter(|active| active.owner_uid == target_uid && active.buff.buff_id == Some(buff_id))
        {
            if active.buff.duration == Some(duration) {
                continue;
            }
            let before = active.buff.clone();
            active.buff.duration = Some(duration);
            refreshed.push(BuffUpdateResult {
                target_uid,
                before,
                after: active.buff.clone(),
            });
        }
        BuffReplaceResult {
            removed: Vec::new(),
            added: None,
            refreshed,
            rejected: None,
            fanout: Vec::new(),
        }
    }

    pub(crate) fn add_special_count(
        &mut self,
        owner_uid: i64,
        marker_ids: &[i32],
        count: i32,
    ) -> Vec<BuffUpdateResult> {
        if owner_uid == 0 || count <= 0 {
            return Vec::new();
        }
        let mut changed = Vec::new();
        for (index, marker_id) in marker_ids.iter().copied().enumerate() {
            if marker_id <= 0 || marker_ids[..index].contains(&marker_id) {
                continue;
            }
            let Some(active) = self.buffs.iter_mut().find(|active| {
                active.owner_uid == owner_uid
                    && (active.type_id == marker_id || active.buff.buff_id == Some(marker_id))
            }) else {
                continue;
            };
            let before = active.buff.clone();
            let value = special_count_value(active.buff.act_common_params.as_deref()) + count;
            active.buff.act_common_params = Some(format!("1003#{value}"));
            changed.push(BuffUpdateResult {
                target_uid: owner_uid,
                before,
                after: active.buff.clone(),
            });
        }
        changed
    }

    pub(super) fn set_state(
        &mut self,
        target_uid: i64,
        buff_uid: i64,
        ex_info: Option<i32>,
        params: Option<String>,
        act_info: Option<Vec<BuffActInfo>>,
    ) -> Option<BuffUpdateResult> {
        let active = self
            .buffs
            .iter_mut()
            .find(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))?;
        let before = active.buff.clone();
        if let Some(ex_info) = ex_info {
            active.buff.ex_info = Some(ex_info);
        }
        if let Some(params) = params {
            active.buff.act_common_params = Some(params);
        }
        if let Some(act_info) = act_info {
            active.buff.act_info = act_info;
        }
        Some(BuffUpdateResult {
            target_uid,
            before,
            after: active.buff.clone(),
        })
    }

    pub fn snapshot(&self, target_uid: i64, buff_uid: i64) -> Option<BuffInfo> {
        self.buffs
            .iter()
            .find(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))
            .map(|active| active.buff.clone())
    }

    pub(crate) fn update(
        &mut self,
        target_uid: i64,
        buff_uid: i64,
        layer: Option<i32>,
        count: Option<i32>,
        duration: Option<i32>,
    ) -> Option<BuffUpdateResult> {
        let active = self
            .buffs
            .iter_mut()
            .find(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))?;
        let before = active.buff.clone();
        let before_amount = count_or_layer_from(&before, active.definition.as_ref());

        if let Some(layer) = layer {
            active.buff.layer = Some(layer);
        }
        if let Some(count) = count {
            active.buff.count = Some(count);
        }
        if let Some(duration) = duration {
            active.buff.duration = Some(duration);
        }
        let after = active.buff.clone();
        let added =
            (count_or_layer_from(&after, active.definition.as_ref()) - before_amount).max(0);
        let result = BuffUpdateResult {
            target_uid,
            before,
            after,
        };
        if let Some(buff_id) = result.after.buff_id {
            self.record_added(target_uid, buff_id, added);
        }
        Some(result)
    }

    #[cfg(test)]
    pub(crate) fn consume_by_type_or_id(
        &mut self,
        target_uid: i64,
        type_or_buff_id: i32,
        amount: i32,
    ) -> Option<BuffUpdateResult> {
        let action = self.resolve_consume_action(
            target_uid,
            BuffSelector::IdOrType(type_or_buff_id),
            amount,
            DepletedBuff::Keep,
            super::command::ConsumeField::Configured,
        );
        self.commit_consume_action(target_uid, action)
            .refreshed
            .into_iter()
            .next()
    }

    #[cfg(test)]
    pub(crate) fn consume_by_type_or_id_replacing(
        &mut self,
        target_uid: i64,
        type_or_buff_id: i32,
        amount: i32,
    ) -> BuffReplaceResult {
        let action = self.resolve_consume_action(
            target_uid,
            BuffSelector::IdOrType(type_or_buff_id),
            amount,
            DepletedBuff::Remove,
            super::command::ConsumeField::Configured,
        );
        self.commit_consume_action(target_uid, action)
    }

    pub(crate) fn delete(&mut self, target_uid: i64, buff_uid: i64) -> Vec<BuffRemoveResult> {
        let Some(index) = self
            .buffs
            .iter()
            .position(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))
        else {
            return Vec::new();
        };
        self.remove_at(index, 0)
    }

    #[cfg(test)]
    pub(crate) fn remove_by_id(&mut self, target_uid: i64, buff_id: i32) -> Vec<BuffRemoveResult> {
        let mut removed = Vec::new();
        while let Some(index) = self.buffs.iter().position(|active| {
            active.owner_uid == target_uid && active.buff.buff_id == Some(buff_id)
        }) {
            removed.extend(self.remove_at(index, 0));
        }
        removed
    }

    #[cfg(test)]
    pub(crate) fn remove_by_statuses(
        &mut self,
        target_uid: i64,
        statuses: &[BuffStatus],
        count: i32,
        config_effect: i32,
    ) -> Vec<BuffRemoveResult> {
        let mut removed = Vec::new();
        let mut removed_count = 0;
        while (count <= 0 || removed_count < count)
            && let Some(index) = self.buffs.iter().position(|active| {
                active.owner_uid == target_uid
                    && active
                        .definition
                        .as_ref()
                        .is_some_and(|definition| statuses.contains(&definition.status))
            })
        {
            removed.extend(self.remove_at(index, config_effect));
            removed_count += 1;
        }
        removed
    }

    pub(super) fn remove_at(&mut self, index: usize, config_effect: i32) -> Vec<BuffRemoveResult> {
        let active = self.buffs.remove(index);
        let clears_count = active
            .definition
            .as_ref()
            .is_some_and(|definition| definition.uses_typed_count());
        let before_amount = count_or_layer_from(&active.buff, active.definition.as_ref());
        let recorded_mode_hp_delta = active
            .buff
            .uid
            .map(|buff_uid| {
                self.act_value(
                    buff_uid,
                    crate::engine::skill::buff_act::rouge2_attr_to_role::ACT_ID,
                )
            })
            .unwrap_or_default();
        if let Some(buff_uid) = active.buff.uid {
            self.remove_act_states(buff_uid);
        }
        let owner_uid = active.owner_uid;
        let linked_ids = active
            .buff
            .buff_id
            .into_iter()
            .flat_map(|buff_id| halo::carriers(self.catalog(), buff_id))
            .filter_map(|carrier| carrier.linked_buff_id)
            .collect::<Vec<_>>();
        let mut removed = Vec::new();

        for linked_id in linked_ids {
            let still_owned = self.buffs.iter().any(|candidate| {
                candidate.owner_uid == owner_uid
                    && candidate
                        .buff
                        .buff_id
                        .into_iter()
                        .flat_map(|buff_id| halo::carriers(self.catalog(), buff_id))
                        .any(|carrier| carrier.linked_buff_id == Some(linked_id))
            });
            if still_owned {
                continue;
            }
            while let Some(child_index) = self.buffs.iter().position(|candidate| {
                candidate.buff.buff_id == Some(linked_id)
                    && candidate.buff.from_uid == Some(owner_uid)
            }) {
                removed.extend(self.remove_at(child_index, config_effect));
            }
        }

        let fixed_max_hp_delta =
            super::query::fixed_attribute_value(&active, crate::engine::entity::attr::AttrId::Hp)
                .filter(|delta| *delta != 0)
                .unwrap_or(recorded_mode_hp_delta);
        let mut buff = active.buff;
        if clears_count {
            buff.count = Some(0);
        }
        removed.push(BuffRemoveResult {
            target_uid: owner_uid,
            before_amount,
            buff,
            config_effect,
            delete_reason: None,
            depleted: false,
            fixed_max_hp_delta,
        });
        removed
    }
}

pub(super) fn special_count_value(raw: Option<&str>) -> i32 {
    let Some((act_id, value)) = raw.and_then(|raw| raw.split_once('#')) else {
        return 0;
    };
    if act_id.trim() != "1003" {
        return 0;
    }
    value.trim().parse().unwrap_or_default()
}
