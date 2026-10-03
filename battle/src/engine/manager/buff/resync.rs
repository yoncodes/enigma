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
        if let Some(active) = self
            .buffs
            .iter_mut()
            .find(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))
        {
            if active.buff == buff {
                return Ok(false);
            }
            active.buff = buff;
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
        self.buffs.push(ActiveBuff {
            owner_uid: target_uid,
            team_type,
            type_id,
            definition,
            buff,
        });
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
