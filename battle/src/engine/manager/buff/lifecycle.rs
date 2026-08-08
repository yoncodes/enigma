use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum BuffTakeActionTrigger {
    OwnerCastSkill = 9,
    OwnerAttackedBySkill = 10,
}

impl BuffTakeActionTrigger {
    fn parse(value: &str) -> Option<Self> {
        match value.parse().ok()? {
            9 => Some(Self::OwnerCastSkill),
            10 => Some(Self::OwnerAttackedBySkill),
            _ => None,
        }
    }

    pub(crate) fn key(self) -> crate::engine::skill::rule::DefinitionKey {
        match self {
            Self::OwnerCastSkill => {
                crate::engine::skill::rule::DefinitionKey::new(self as i32, "OwnerCastSkillSlot")
            }
            Self::OwnerAttackedBySkill => crate::engine::skill::rule::DefinitionKey::new(
                self as i32,
                "OwnerAttackedBySkillSlot",
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuffTakeAction {
    pub(crate) trigger: BuffTakeActionTrigger,
    pub(crate) skill_slots: Vec<i32>,
}

impl BuffTakeAction {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let (trigger, slots) = value.split_once('#')?;
        let trigger = BuffTakeActionTrigger::parse(trigger)?;
        let skill_slots = slots
            .split([',', '，'])
            .filter_map(|slot| slot.trim().parse().ok())
            .collect::<Vec<_>>();
        (!skill_slots.is_empty()).then_some(Self {
            trigger,
            skill_slots,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuffActionExpiry {
    pub(crate) owner_uid: i64,
    pub(crate) source_uid: i64,
    pub(crate) buff_uid: i64,
    pub(crate) buff_id: i32,
    pub(crate) trigger: BuffTakeActionTrigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuffDurationPlan {
    target_uid: i64,
    buff_uid: i64,
    take_stage: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuffLifecyclePlan {
    pub(crate) target_uid: i64,
    pub(crate) buff_uid: i64,
}

impl BuffManager {
    pub(crate) fn plan_action_expiries(
        &self,
        action: &crate::engine::skill::action::SkillActionEvent,
    ) -> Vec<BuffActionExpiry> {
        if action.phase != crate::engine::skill::action::SkillPhase::HitPassives {
            return Vec::new();
        }

        self.buffs
            .iter()
            .filter_map(|active| {
                let definition = active.definition.as_ref()?;
                if definition.keeps_permanent_instance() {
                    return None;
                }
                let take_action = definition.take_action()?;
                if !take_action.skill_slots.contains(&action.skill_slot) {
                    return None;
                }
                let matches = match take_action.trigger {
                    BuffTakeActionTrigger::OwnerCastSkill => active.owner_uid == action.source_uid,
                    BuffTakeActionTrigger::OwnerAttackedBySkill => {
                        action.is_attack
                            && (action.target_uid == active.owner_uid
                                || action.target_uids.contains(&active.owner_uid))
                    }
                };
                matches.then_some(BuffActionExpiry {
                    owner_uid: active.owner_uid,
                    source_uid: active.buff.from_uid.unwrap_or(active.owner_uid),
                    buff_uid: active.buff.uid?,
                    buff_id: active.buff.buff_id?,
                    trigger: take_action.trigger,
                })
            })
            .collect()
    }

    pub(crate) fn seed(&mut self, fight: &Fight) {
        self.buffs.clear();
        self.entities.clear();
        self.team_types.clear();
        self.added_by_owner.clear();
        self.added_by_team.clear();
        self.added_history.clear();
        self.act_states.clear();
        self.act_values.clear();
        self.grant_values.clear();
        let fight_version = fight.version.unwrap_or_default();
        self.shared_uid_lane = uid::uses_shared_lane(fight_version);
        self.attacker = BuffUidAllocator::new(uid::attacker_start(fight_version));
        self.defender = BuffUidAllocator::new(DEFENDER_BUFF_UID_START);

        if let Some(team) = &fight.attacker {
            self.seed_team(team, 1);
        }
        if let Some(team) = &fight.defender {
            self.seed_team(team, 2);
        }
        self.seed_grant_values();
    }

    pub(crate) fn sync_entity(&self, entity: &mut FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        entity.buffs = self.active_for(uid).cloned().collect();
    }

    pub(crate) fn project_synchronization(
        &self,
        entity: &mut FightEntityInfo,
        progress: crate::engine::manager::ex_point::SynchronizationProgress,
    ) {
        let Some(owner_uid) = entity.uid else { return };
        let Some((buff_uid, act_id, _)) = self.buff_act_carrier(
            owner_uid,
            crate::engine::skill::buff_act::registry::BuffActKind::EzioBigSkill,
        ) else {
            return;
        };
        let Ok(target_uid) = i32::try_from(progress.target_uid) else {
            return;
        };
        let params = vec![
            progress.completed_actions.saturating_add(1),
            progress.total_damage,
            target_uid,
        ];
        let Some(buff) = entity
            .buffs
            .iter_mut()
            .find(|buff| buff.uid == Some(buff_uid))
        else {
            return;
        };
        buff.act_common_params = Some(format!(
            "{act_id}#{}",
            params
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
        if let Some(info) = buff
            .act_info
            .iter_mut()
            .find(|info| info.act_id == Some(act_id))
        {
            info.param = params;
        } else {
            buff.act_info.push(BuffActInfo {
                act_id: Some(act_id),
                param: params,
                str_param: Some(String::new()),
            });
        }
    }

    pub(crate) fn plan_round_start_duration_sync(
        &self,
        owner_uids: &[i64],
    ) -> Vec<BuffLifecyclePlan> {
        self.buffs
            .iter()
            .filter_map(|active| {
                if active.team_type != 1 || !owner_uids.contains(&active.owner_uid) {
                    return None;
                }

                let definition = active.definition.as_ref()?;
                if definition.duration <= 0 {
                    return None;
                }

                let should_sync = active.buff.layer.unwrap_or_default() > 1
                    || active.buff.count.unwrap_or_default() > 1
                    || definition.syncs_single_stack_duration();
                if !should_sync {
                    return None;
                }
                Some(BuffLifecyclePlan {
                    target_uid: active.owner_uid,
                    buff_uid: active.buff.uid?,
                })
            })
            .collect()
    }

    pub(crate) fn commit_round_start_duration_sync(
        &mut self,
        plan: BuffLifecyclePlan,
    ) -> Option<BuffUpdateResult> {
        let index = self.buffs.iter().position(|active| {
            active.owner_uid == plan.target_uid && active.buff.uid == Some(plan.buff_uid)
        })?;
        let before = self.buffs[index].buff.clone();
        let next_duration = before.duration.unwrap_or_default().saturating_sub(1);
        self.buffs[index].buff.duration = Some(next_duration);
        Some(BuffUpdateResult {
            target_uid: plan.target_uid,
            before,
            after: self.buffs[index].buff.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn advance_durations(&mut self, take_stage: i32) -> Vec<BuffReplaceResult> {
        self.advance_durations_for(take_stage, &[])
    }

    #[cfg(test)]
    pub(crate) fn advance_durations_for(
        &mut self,
        take_stage: i32,
        owner_uids: &[i64],
    ) -> Vec<BuffReplaceResult> {
        let plans = self.plan_duration_advances(take_stage, owner_uids);
        plans
            .into_iter()
            .filter_map(|plan| self.commit_duration_advance(plan))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn advance_durations_for_snapshot(
        &mut self,
        take_stage: i32,
        owner_uids: &[i64],
        buff_uids: &[i64],
    ) -> Vec<BuffReplaceResult> {
        let plans = self.plan_duration_advances_matching(take_stage, owner_uids, Some(buff_uids));
        plans
            .into_iter()
            .filter_map(|plan| self.commit_duration_advance(plan))
            .collect()
    }

    pub(crate) fn plan_duration_advances(
        &self,
        take_stage: i32,
        owner_uids: &[i64],
    ) -> Vec<BuffDurationPlan> {
        self.plan_duration_advances_matching(take_stage, owner_uids, None)
    }

    pub(crate) fn duration_buff_uids(&self, take_stage: i32, owner_uids: &[i64]) -> Vec<i64> {
        self.plan_duration_advances(take_stage, owner_uids)
            .into_iter()
            .map(|plan| plan.buff_uid)
            .collect()
    }

    pub(crate) fn plan_duration_advances_for_snapshot(
        &self,
        take_stage: i32,
        owner_uids: &[i64],
        buff_uids: &[i64],
    ) -> Vec<BuffDurationPlan> {
        self.plan_duration_advances_matching(take_stage, owner_uids, Some(buff_uids))
    }

    fn plan_duration_advances_matching(
        &self,
        take_stage: i32,
        owner_uids: &[i64],
        buff_uids: Option<&[i64]>,
    ) -> Vec<BuffDurationPlan> {
        let mut indices = if owner_uids.is_empty() {
            (0..self.buffs.len()).collect::<Vec<_>>()
        } else {
            owner_uids
                .iter()
                .flat_map(|owner_uid| {
                    self.buffs
                        .iter()
                        .enumerate()
                        .filter(move |(_, active)| active.owner_uid == *owner_uid)
                        .map(|(index, _)| index)
                })
                .collect()
        };
        if !owner_uids.is_empty() {
            indices.sort_by_key(|index| {
                let active = &self.buffs[*index];
                (
                    owner_uids
                        .iter()
                        .position(|uid| *uid == active.owner_uid)
                        .unwrap_or(usize::MAX),
                    active.buff.uid.unwrap_or_default(),
                )
            });
        }
        indices
            .into_iter()
            .filter_map(|index| {
                let active = &self.buffs[index];
                let buff_uid = active.buff.uid?;
                let duration = active.buff.duration.unwrap_or_default();
                (duration > 0
                    && active
                        .definition
                        .as_ref()
                        .is_some_and(|definition| definition.take_stage == take_stage)
                    && buff_uids.is_none_or(|buff_uids| buff_uids.contains(&buff_uid)))
                .then_some(BuffDurationPlan {
                    target_uid: active.owner_uid,
                    buff_uid,
                    take_stage,
                })
            })
            .collect()
    }

    pub(crate) fn commit_duration_advance(
        &mut self,
        plan: BuffDurationPlan,
    ) -> Option<BuffReplaceResult> {
        let index = self.buffs.iter().position(|active| {
            active.owner_uid == plan.target_uid && active.buff.uid == Some(plan.buff_uid)
        })?;
        let active = &self.buffs[index];
        let duration = active.buff.duration.unwrap_or_default();
        if duration <= 0
            || active
                .definition
                .as_ref()
                .is_none_or(|definition| definition.take_stage != plan.take_stage)
        {
            return None;
        }
        let before = active.buff.clone();
        self.buffs[index].buff.duration = Some(duration - 1);
        let after = self.buffs[index].buff.clone();
        if duration == 1 {
            Some(BuffReplaceResult {
                removed: self.delete(plan.target_uid, plan.buff_uid),
                ..Default::default()
            })
        } else {
            Some(BuffReplaceResult {
                refreshed: vec![BuffUpdateResult {
                    target_uid: plan.target_uid,
                    before,
                    after,
                }],
                ..Default::default()
            })
        }
    }

    fn seed_team(&mut self, team: &FightTeam, fallback_team_type: i32) {
        for entity in &team.entitys {
            self.seed_entity(entity, fallback_team_type, true);
        }
        for entity in &team.sub_entitys {
            self.seed_entity(entity, fallback_team_type, false);
        }
        if let Some(entity) = &team.assist_boss {
            let team_type = entity.team_type.unwrap_or(fallback_team_type);
            if let Some(uid) = entity.uid {
                self.team_types.insert(uid, team_type);
            }
            self.seed_buffs(entity, team_type);
        }
    }

    fn seed_entity(&mut self, entity: &FightEntityInfo, fallback_team_type: i32, active: bool) {
        let Some(uid) = entity.uid else { return };
        let team_type = entity.team_type.unwrap_or(fallback_team_type);
        self.team_types.insert(uid, team_type);
        self.entities.push(TrackedEntity {
            uid,
            team_type,
            active,
        });
        self.seed_buffs(entity, team_type);
    }

    fn seed_buffs(&mut self, entity: &FightEntityInfo, team_type: i32) {
        let Some(uid) = entity.uid else { return };
        for buff in entity.buffs.iter().cloned() {
            if let Some(uid) = buff.uid {
                self.allocator_for(team_type).observe(uid);
            }
            let definition = BuffDefinition::get(buff.buff_id.unwrap_or_default());
            let type_id = definition
                .as_ref()
                .map(BuffDefinition::effective_type_id)
                .unwrap_or_else(|| fallback_type_id(&buff));
            self.buffs.push(ActiveBuff {
                owner_uid: uid,
                team_type,
                type_id,
                definition,
                buff,
            });
        }
    }

    pub(crate) fn register_entity(&mut self, entity: &FightEntityInfo, fallback_team_type: i32) {
        let Some(uid) = entity.uid else { return };
        self.entities.retain(|tracked| tracked.uid != uid);
        self.team_types.remove(&uid);
        self.buffs.retain(|active| active.owner_uid != uid);
        self.seed_entity(entity, fallback_team_type, true);
    }

    pub(crate) fn sync_roster(&mut self, fight: &Fight) {
        for tracked in &mut self.entities {
            tracked.active = fight
                .attacker
                .iter()
                .chain(fight.defender.iter())
                .flat_map(|team| &team.entitys)
                .any(|entity| entity.uid == Some(tracked.uid));
        }
    }
}
