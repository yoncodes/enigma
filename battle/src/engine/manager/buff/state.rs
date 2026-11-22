use crate::engine::skill::buff_act::registry::{BuffActKind, StatReadTiming};

use super::{BuffAddArgs, BuffDefinition, BuffManager};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DotState {
    contributions: Vec<DotContribution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DotContribution {
    source_uid: i64,
    damage_per_stack: i32,
    stacks: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DotSnapshotPlan {
    act_id: i32,
    contribution: DotContribution,
}

impl BuffManager {
    pub fn act_value(&self, buff_uid: i64, act_id: i32) -> i32 {
        self.act_values
            .get(&(buff_uid, act_id))
            .copied()
            .unwrap_or_default()
    }

    pub fn grant_value(&self, buff_uid: i64, act_id: i32) -> Option<i32> {
        self.grant_values.get(&(buff_uid, act_id)).copied()
    }

    pub fn attr_snapshot(
        &self,
        buff_uid: i64,
        owner_uid: i64,
        attr_id: crate::engine::entity::attr::AttrId,
    ) -> Option<i32> {
        self.attr_snapshots
            .get(&(buff_uid, owner_uid, attr_id))
            .copied()
    }

    pub(super) fn accumulate_act_value(&mut self, buff_uid: i64, act_id: i32, delta: i32) {
        *self.act_values.entry((buff_uid, act_id)).or_default() += delta;
    }

    pub(super) fn plan_grant_values(
        &self,
        definition: &BuffDefinition,
        source_uid: i64,
    ) -> Vec<(i32, i32)> {
        definition
            .features()
            .iter()
            .filter(|feature| feature.stat_read_timing == StatReadTiming::OnGrant)
            .filter_map(|feature| {
                let [act_id, args @ ..] = feature.values.as_slice() else {
                    return None;
                };
                match feature.kind {
                    Some(BuffActKind::FixTempAttrByBuffLayer)
                        if matches!(args, [_, 1, ..]) =>
                    {
                        Some((
                            *act_id,
                            self.buff_id_or_type_amount(source_uid, args[0]),
                        ))
                    }
                    Some(BuffActKind::TeamExElectricTransConsumeValueAttr) => {
                        let rule = crate::engine::skill::buff_act::electric_transform::team_attribute_rule(args)?;
                        let recorded = self.buff_act_value(
                            source_uid,
                            BuffActKind::RecordTeamExElectricTransConsumeValue,
                        );
                        Some((
                            *act_id,
                            recorded.saturating_mul(rule.per_recorded_stack),
                        ))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    pub(super) fn commit_grant_values(&mut self, buff_uid: i64, snapshots: &[(i32, i32)]) {
        for &(act_id, value) in snapshots {
            self.grant_values.insert((buff_uid, act_id), value);
        }
    }

    pub(crate) fn commit_attr_snapshots(
        &mut self,
        buff_uid: i64,
        snapshots: &[(i64, crate::engine::entity::attr::AttrId, i32)],
    ) {
        for &(owner_uid, attr_id, value) in snapshots {
            self.attr_snapshots
                .insert((buff_uid, owner_uid, attr_id), value);
        }
    }

    pub(super) fn seed_grant_values(&mut self) {
        let buffs = self
            .buffs
            .iter()
            .filter_map(|active| {
                Some((
                    active.buff.uid?,
                    active.buff.from_uid.unwrap_or(active.owner_uid),
                    active.definition.clone()?,
                ))
            })
            .collect::<Vec<_>>();
        for (buff_uid, source_uid, definition) in buffs {
            let snapshots = self.plan_grant_values(&definition, source_uid);
            self.commit_grant_values(buff_uid, &snapshots);
        }
    }

    pub(super) fn plan_grant_snapshots(
        definition: &BuffDefinition,
        source_uid: i64,
        source_attack: Option<i32>,
        source_max_hp: Option<i32>,
        args: BuffAddArgs,
        new_instance: bool,
    ) -> Vec<DotSnapshotPlan> {
        let stacks = definition
            .layer(args.layer, args.layer_specified, args.count)
            .max(1);
        definition
            .features()
            .iter()
            .filter(|feature| {
                matches!(
                    feature.stat_read_timing,
                    StatReadTiming::OnGrant | StatReadTiming::ByArguments
                )
            })
            .filter_map(|feature| {
                let [act_id, args @ ..] = feature.values.as_slice() else {
                    return None;
                };
                let (source_attribute, permille, stacks) = match (feature.kind, args) {
                    (Some(BuffActKind::Poison), [permille, ..]) => {
                        (source_attack?, *permille, stacks)
                    }
                    (Some(BuffActKind::Dot), [0, attr_id, permille]) => {
                        match crate::engine::entity::attr::AttrId::from_raw(*attr_id) {
                            Some(crate::engine::entity::attr::AttrId::Attack) => {
                                (source_attack?, *permille, stacks)
                            }
                            Some(crate::engine::entity::attr::AttrId::Hp) if new_instance => {
                                (source_max_hp?, *permille, 1)
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                };
                (permille > 0).then_some(DotSnapshotPlan {
                    act_id: *act_id,
                    contribution: DotContribution {
                        source_uid,
                        damage_per_stack: crate::engine::damage::attribute_scaled_damage(
                            source_attribute,
                            permille,
                            1,
                        ),
                        stacks,
                    },
                })
            })
            .collect()
    }

    pub(super) fn plan_grant_params(
        &self,
        definition: &BuffDefinition,
        source_uid: i64,
    ) -> Option<String> {
        let feature = definition.features().iter().find(|feature| {
            feature.stat_read_timing == StatReadTiming::OnGrant
                && matches!(
                    feature.kind,
                    Some(BuffActKind::AttrFixFromInjuryBank | BuffActKind::ModifyAttrByBuffLayer)
                )
        })?;
        let [act_id, args @ ..] = feature.values.as_slice() else {
            return None;
        };
        match (feature.kind, args) {
            (Some(BuffActKind::ModifyAttrByBuffLayer), _) => {
                crate::engine::skill::buff_act::modify_attr_by_buff_layer::grant_params(
                    *act_id, args, source_uid, self,
                )
            }
            (Some(BuffActKind::AttrFixFromInjuryBank), [_attr_id, injury_rate, base, ..]) => {
                let injury = self.injury_bank_value(source_uid)?;
                Some(format!(
                    "{act_id}#{}",
                    base.saturating_add(injury.saturating_mul(*injury_rate) / 1000)
                ))
            }
            _ => None,
        }
    }

    fn injury_bank_value(&self, owner_uid: i64) -> Option<i32> {
        let active = self.buffs.iter().find(|active| {
            active.owner_uid == owner_uid
                && active.definition.as_ref().is_some_and(|definition| {
                    definition
                        .features()
                        .iter()
                        .any(|feature| feature.kind == Some(BuffActKind::InjuryBank))
                })
        })?;
        let mut parts = active.buff.act_common_params.as_deref()?.split('#');
        let act_id = parts.next()?.parse::<i32>().ok()?;
        let value = parts.next()?.parse::<i32>().ok()?;
        let is_injury_bank = active
            .definition
            .as_ref()?
            .features()
            .iter()
            .any(|feature| {
                feature.kind == Some(BuffActKind::InjuryBank)
                    && feature.values.first() == Some(&act_id)
            });
        is_injury_bank.then_some(value)
    }

    pub(super) fn commit_grant_snapshots(&mut self, buff_uid: i64, plans: &[DotSnapshotPlan]) {
        for plan in plans {
            let state = self.act_states.entry((buff_uid, plan.act_id)).or_default();
            state.contributions.push(plan.contribution);
        }
    }

    pub fn dot_damage_for_owner(
        &self,
        owner_uid: i64,
        act_id: i32,
        triggering_buff_uid: i64,
    ) -> Option<i32> {
        self.buffs.iter().find(|active| {
            active.owner_uid == owner_uid && active.buff.uid == Some(triggering_buff_uid)
        })?;
        let state = self.act_states.get(&(triggering_buff_uid, act_id))?;
        Some(
            state
                .contributions
                .iter()
                .map(|contribution| contribution.damage_per_stack * contribution.stacks)
                .sum(),
        )
    }

    pub fn dot_remaining_damage_for_owner(
        &self,
        owner_uid: i64,
        act_id: i32,
        triggering_buff_uid: i64,
    ) -> Option<i32> {
        let active = self.buffs.iter().find(|active| {
            active.owner_uid == owner_uid && active.buff.uid == Some(triggering_buff_uid)
        })?;
        let duration = active.buff.duration.unwrap_or(1).max(1);
        let state = self.act_states.get(&(triggering_buff_uid, act_id))?;
        Some(
            state
                .contributions
                .iter()
                .map(|contribution| {
                    contribution
                        .damage_per_stack
                        .saturating_mul(contribution.stacks)
                        .saturating_mul(duration)
                })
                .sum(),
        )
    }

    pub(super) fn remove_act_states(&mut self, buff_uid: i64) {
        self.act_states
            .retain(|(state_buff_uid, _), _| *state_buff_uid != buff_uid);
        self.act_values
            .retain(|(state_buff_uid, _), _| *state_buff_uid != buff_uid);
        self.grant_values
            .retain(|(state_buff_uid, _), _| *state_buff_uid != buff_uid);
        self.attr_snapshots
            .retain(|(state_buff_uid, _, _), _| *state_buff_uid != buff_uid);
    }
}

#[cfg(test)]
mod tests;
