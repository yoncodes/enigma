use super::*;

impl BuffManager {
    /// Preview diagnostics only: reconciles one captured wire buff without emitting gameplay
    /// events or applying grant/removal policy.
    pub(crate) fn resync_observed_present(
        &mut self,
        target_uid: i64,
        buff: BuffInfo,
    ) -> Result<bool, String> {
        let buff_uid = buff
            .uid
            .filter(|uid| *uid > 0)
            .ok_or_else(|| format!("captured buff on target {target_uid} has no valid uid"))?;
        let buff_id = buff.buff_id.filter(|id| *id > 0).ok_or_else(|| {
            format!("captured buff uid={buff_uid} on target {target_uid} has no valid buff id")
        })?;
        let mut changed = false;
        if let Ok(policy) = BuffPolicy::configured(self.catalog().game_data(), buff_id)
            && policy.on_duplicate != DuplicateGrant::AddSeparateCopy
        {
            let stale_uids = self
                .buffs
                .iter()
                .filter(|active| {
                    active.owner_uid == target_uid
                        && active.buff.uid != Some(buff_uid)
                        && match policy.match_existing {
                            ExistingBuffMatch::SameId => active.buff.buff_id == Some(buff_id),
                            ExistingBuffMatch::SameIdAndDuration => {
                                active.buff.buff_id == Some(buff_id)
                                    && active.buff.duration == buff.duration
                            }
                            ExistingBuffMatch::SharedTypeFamily => {
                                active.type_id == policy.effective_type_id
                                    && active
                                        .definition
                                        .as_ref()
                                        .is_some_and(BuffDefinition::matches_type_family)
                            }
                            ExistingBuffMatch::SameType => {
                                active.type_id == policy.effective_type_id
                            }
                        }
                })
                .filter_map(|active| active.buff.uid)
                .collect::<Vec<_>>();
            changed = !stale_uids.is_empty();
            self.buffs.retain(|active| {
                !stale_uids
                    .iter()
                    .any(|uid| active.owner_uid == target_uid && active.buff.uid == Some(*uid))
            });
            for stale_uid in stale_uids {
                self.remove_act_states(stale_uid);
            }
        }
        if let Some(index) = self
            .buffs
            .iter()
            .position(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))
        {
            if self.buffs[index].buff == buff {
                return Ok(changed);
            }
            let semantic_identity_changed = self.buffs[index].buff.buff_id != Some(buff_id)
                || self.buffs[index].buff.from_uid != buff.from_uid;
            if semantic_identity_changed {
                let definition = BuffDefinition::configured(self.catalog().game_data(), buff_id);
                let type_id = definition
                    .as_ref()
                    .map(BuffDefinition::effective_type_id)
                    .unwrap_or_else(|| fallback_type_id(&buff));
                let source_uid = buff.from_uid.unwrap_or(target_uid);
                self.remove_act_states(buff_uid);
                let grant_values = definition
                    .as_ref()
                    .map(|definition| self.plan_grant_values(definition, source_uid))
                    .unwrap_or_default();
                self.buffs[index].definition = definition;
                self.buffs[index].type_id = type_id;
                self.buffs[index].fanout_emitter_uid = None;
                self.buffs[index].fanout_carrier_uid = None;
                self.buffs[index].effective = true;
                self.commit_grant_values(buff_uid, &grant_values);
            }
            self.buffs[index].buff = buff;
            return Ok(true);
        }

        let team_type = self
            .team_type(target_uid)
            .ok_or_else(|| format!("captured buff target uid={target_uid} is not in the roster"))?;
        let definition = BuffDefinition::configured(self.catalog().game_data(), buff_id);
        let type_id = definition
            .as_ref()
            .map(BuffDefinition::effective_type_id)
            .unwrap_or_else(|| fallback_type_id(&buff));
        self.allocator_for(team_type).observe(buff_uid);
        let source_uid = buff.from_uid.unwrap_or(target_uid);
        let grant_values = definition
            .as_ref()
            .map(|definition| self.plan_grant_values(definition, source_uid))
            .unwrap_or_default();
        self.buffs.push(ActiveBuff {
            owner_uid: target_uid,
            team_type,
            type_id,
            definition,
            fanout_emitter_uid: None,
            fanout_carrier_uid: None,
            array_owner: None,
            effective: true,
            buff,
        });
        self.commit_grant_values(buff_uid, &grant_values);
        Ok(true)
    }

    /// Preview diagnostics only: removes exactly the captured wire instance.
    pub(crate) fn resync_observed_removed(&mut self, target_uid: i64, buff_uid: i64) -> bool {
        let Some(index) = self
            .buffs
            .iter()
            .position(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))
        else {
            return false;
        };
        self.buffs.remove(index);
        self.remove_act_states(buff_uid);
        true
    }

    pub(crate) fn finish_observed_resync(&mut self) {
        self.reconcile_transition_progress();
    }
}

#[cfg(test)]
mod tests;
