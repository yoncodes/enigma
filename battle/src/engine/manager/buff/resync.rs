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
        if let Some(index) = self
            .buffs
            .iter()
            .position(|active| active.owner_uid == target_uid && active.buff.uid == Some(buff_uid))
        {
            if self.buffs[index].buff == buff {
                return Ok(false);
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
mod tests {
    use super::*;
    use sonettobuf::{FightEntityInfo, FightTeam};

    fn manager(target_buff: Option<BuffInfo>) -> BuffManager {
        crate::test_support::init_config();
        let mut manager = BuffManager::default();
        manager.seed(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        buffs: vec![BuffInfo {
                            uid: Some(99),
                            buff_id: Some(31430145),
                            from_uid: Some(10),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        team_type: Some(1),
                        buffs: target_buff.into_iter().collect(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        });
        manager.accumulate_act_value(99, 1128, 7);
        manager
    }

    fn observed_snapshot_buff() -> BuffInfo {
        BuffInfo {
            uid: Some(1),
            buff_id: Some(31430131),
            from_uid: Some(10),
            ..Default::default()
        }
    }

    #[test]
    fn observed_insert_initializes_grant_time_state() {
        let mut manager = manager(None);

        assert!(
            manager
                .resync_observed_present(20, observed_snapshot_buff())
                .unwrap()
        );

        assert_eq!(manager.grant_value(1, 1127), Some(140));
    }

    #[test]
    fn observed_id_change_rebuilds_definition_and_grant_time_state() {
        let mut manager = manager(Some(BuffInfo {
            uid: Some(1),
            buff_id: Some(31050111),
            from_uid: Some(20),
            ..Default::default()
        }));

        assert!(
            manager
                .resync_observed_present(20, observed_snapshot_buff())
                .unwrap()
        );

        assert_eq!(manager.grant_value(1, 1127), Some(140));
        assert_eq!(manager.buff_id_amount(20, 31050111), 0);
        assert_eq!(manager.buff_id_amount(20, 31430131), 1);
    }
}
