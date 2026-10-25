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
mod tests {
    use super::*;
    use sonettobuf::{FightEntityInfo, FightTeam};

    fn manager(target_buff: Option<BuffInfo>) -> BuffManager {
        manager_with_buffs(target_buff.into_iter().collect())
    }

    fn manager_with_buffs(target_buffs: Vec<BuffInfo>) -> BuffManager {
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
                        buffs: target_buffs,
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

    #[test]
    fn observed_single_copy_replaces_the_generated_uid() {
        let mut manager = manager(Some(BuffInfo {
            uid: Some(1),
            buff_id: Some(109320111),
            from_uid: Some(20),
            layer: Some(1),
            ..Default::default()
        }));

        assert!(
            manager
                .resync_observed_present(
                    20,
                    BuffInfo {
                        uid: Some(2),
                        buff_id: Some(109320111),
                        from_uid: Some(20),
                        layer: Some(2),
                        ..Default::default()
                    }
                )
                .unwrap()
        );

        assert!(manager.snapshot(20, 1).is_none());
        assert_eq!(manager.snapshot(20, 2).unwrap().layer, Some(2));
        assert_eq!(manager.buff_id_amount(20, 109320111), 2);
    }

    #[test]
    fn observed_single_copy_removes_a_duplicate_beside_the_known_uid() {
        let buff = |uid, layer| BuffInfo {
            uid: Some(uid),
            buff_id: Some(109320111),
            from_uid: Some(20),
            layer: Some(layer),
            ..Default::default()
        };
        let mut manager = manager_with_buffs(vec![buff(1, 1), buff(2, 1)]);

        assert!(manager.resync_observed_present(20, buff(2, 2)).unwrap());

        assert!(manager.snapshot(20, 1).is_none());
        assert_eq!(manager.snapshot(20, 2).unwrap().layer, Some(2));
        assert_eq!(manager.buff_id_amount(20, 109320111), 2);
    }

    #[test]
    fn observed_stale_duplicate_reports_reconciliation() {
        let buff = |uid| BuffInfo {
            uid: Some(uid),
            buff_id: Some(109320111),
            from_uid: Some(20),
            layer: Some(1),
            ..Default::default()
        };
        let mut manager = manager_with_buffs(vec![buff(1), buff(2)]);

        assert!(manager.resync_observed_present(20, buff(2)).unwrap());

        assert!(manager.snapshot(20, 1).is_none());
        assert!(manager.snapshot(20, 2).is_some());
    }

    #[test]
    fn observed_timed_layer_keeps_a_different_duration_copy() {
        let buff = |uid, duration| BuffInfo {
            uid: Some(uid),
            buff_id: Some(7111),
            from_uid: Some(20),
            layer: Some(1),
            duration: Some(duration),
            ..Default::default()
        };
        let mut manager = manager_with_buffs(vec![buff(1, 1), buff(2, 2)]);

        assert!(manager.resync_observed_present(20, buff(3, 1)).unwrap());

        assert!(manager.snapshot(20, 1).is_none());
        assert!(manager.snapshot(20, 2).is_some());
        assert!(manager.snapshot(20, 3).is_some());
    }
}
