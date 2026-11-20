use super::{
    command::{ConsumeAction, ConsumeField, DepletedBuff},
    grant_plan::{PlannedFanout, PlannedFanoutRefresh},
    *,
};

#[derive(Debug, Clone)]
pub(super) struct PlannedCopyTrim {
    owner_uid: i64,
    action: ConsumeAction,
}

#[derive(Debug, Clone, Default)]
pub(super) struct CopyTrimPlan {
    rule: Option<crate::engine::skill::rule::DefinitionKey>,
    emitter_uid: i64,
    carrier_buff_id: i32,
    trimmed_stack_uid: i64,
    trims: Vec<PlannedCopyTrim>,
}

impl PlannedFanout {
    pub(super) fn replaces_layered_copy(&self) -> bool {
        self.replacement_uid.is_some()
            && crate::engine::skill::buff_act::registry::kind(
                self.spec.rule.opcode,
                self.spec.rule.type_name,
            ) == Some(crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo)
    }
}

impl CopyTrimPlan {
    pub(super) fn own_copy_replacements(&mut self, adds: &[&PlannedFanout], carrier_buff_id: i32) {
        if self.rule.is_none()
            && let Some(add) = adds.iter().find(|add| add.replaces_layered_copy())
        {
            self.rule = Some(add.spec.rule);
            self.emitter_uid = add.spec.emitter_uid;
            self.carrier_buff_id = carrier_buff_id;
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.trims.is_empty()
    }

    pub(super) fn owns(&self, rule: crate::engine::skill::rule::DefinitionKey) -> bool {
        !self.is_empty() && self.rule == Some(rule)
    }
}

impl BuffManager {
    pub(super) fn plan_layer_halo_copy_trims(&self, masters: &BuffReplaceResult) -> CopyTrimPlan {
        let mut plan = CopyTrimPlan::default();
        let Some((emitter_uid, carrier_buff_id)) = masters
            .removed
            .iter()
            .map(|removed| (removed.target_uid, removed.buff.buff_id))
            .chain(
                masters
                    .refreshed
                    .iter()
                    .map(|update| (update.target_uid, update.after.buff_id)),
            )
            .find_map(|(target_uid, buff_id)| Some((target_uid, buff_id?)))
        else {
            return plan;
        };
        let Some(carrier) = halo::carriers(self.catalog(), carrier_buff_id)
            .into_iter()
            .find(|carrier| carrier.kind == halo::HaloKind::LayerMaster)
        else {
            return plan;
        };
        plan.rule = Some(crate::engine::skill::rule::DefinitionKey::new(
            carrier.opcode,
            carrier.type_name,
        ));
        plan.emitter_uid = emitter_uid;
        plan.carrier_buff_id = carrier_buff_id;
        let trimmed_stacks = masters
            .removed
            .iter()
            .filter_map(|removed| Some((removed.buff.uid?, None)))
            .chain(masters.refreshed.iter().filter_map(|update| {
                let layers = update.before.layer.unwrap_or_default()
                    - update.after.layer.unwrap_or_default();
                (layers > 0).then_some((update.after.uid?, Some(layers)))
            }))
            .collect::<Vec<(i64, Option<i32>)>>();
        plan.trimmed_stack_uid = trimmed_stacks
            .first()
            .map(|(master_uid, _)| *master_uid)
            .unwrap_or_default();
        for (master_uid, layers) in trimmed_stacks {
            plan.trims.extend(
                self.buffs
                    .iter()
                    .filter(|copy| layer_halo_carrier_uid(copy) == Some(master_uid))
                    .map(|copy| PlannedCopyTrim {
                        owner_uid: copy.owner_uid,
                        action: Self::resolve_active_consume_action(
                            copy,
                            layers.unwrap_or_else(|| {
                                count_or_layer_from(&copy.buff, copy.definition.as_ref())
                            }),
                            DepletedBuff::Remove,
                            ConsumeField::Configured,
                        ),
                    }),
            );
        }
        plan
    }

    pub(super) fn commit_layer_halo_allies(
        &mut self,
        hp: &HpManager,
        carrier_buff_uid: Option<i64>,
        adds: &[&PlannedFanout],
        refreshes: &[&PlannedFanoutRefresh],
        plan: CopyTrimPlan,
    ) -> Option<BuffFanoutResult> {
        let rule = plan.rule?;
        let adds = if carrier_buff_uid.is_some() {
            adds
        } else {
            &[]
        };
        let carrier_buff_uid = carrier_buff_uid
            .or_else(|| refreshes.first().map(|refresh| refresh.carrier_buff_uid))
            .unwrap_or(plan.trimmed_stack_uid);
        let mut allies = self
            .entities
            .iter()
            .map(|entity| entity.uid)
            .filter(|uid| {
                adds.iter().any(|add| add.spec.route.target_uid == *uid)
                    || refreshes
                        .iter()
                        .any(|refresh| refresh.spec.route.target_uid == *uid)
                    || plan.trims.iter().any(|trim| trim.owner_uid == *uid)
            })
            .collect::<Vec<_>>();
        for trim in &plan.trims {
            if !allies.contains(&trim.owner_uid) {
                allies.push(trim.owner_uid);
            }
        }
        let mut results = Vec::new();
        for target_uid in allies {
            let mut ally = LayerHaloAllyResult {
                target_uid,
                part: Default::default(),
                replaced: Vec::new(),
                trimmed: Vec::new(),
                trim_refreshed: Vec::new(),
                added: None,
                refreshed: None,
            };
            for trim in plan
                .trims
                .iter()
                .filter(|trim| trim.owner_uid == target_uid)
            {
                let change = self.commit_consume_action(target_uid, trim.action.clone());
                ally.trimmed.extend(change.removed);
                ally.trim_refreshed.extend(change.refreshed);
            }
            if let Some(add) = adds
                .iter()
                .find(|add| add.spec.route.target_uid == target_uid)
            {
                if let Some(uid) = add.replacement_uid {
                    ally.replaced
                        .extend(self.delete(target_uid, uid).into_iter().map(|mut removed| {
                            removed.buff.duration = Some(0);
                            removed
                        }));
                }
                ally.added = self.commit_fanout_one(hp, Some(carrier_buff_uid), &add.spec, add.uid);
            }
            if let Some(refresh) = refreshes
                .iter()
                .find(|refresh| refresh.spec.route.target_uid == target_uid)
            {
                ally.refreshed = self.commit_fanout_refresh_one(refresh);
            }
            results.push(ally);
        }
        let emitter_uid = adds
            .first()
            .map(|add| add.spec.emitter_uid)
            .or_else(|| refreshes.first().map(|refresh| refresh.spec.emitter_uid))
            .unwrap_or(plan.emitter_uid);
        let carrier_buff_id = refreshes
            .first()
            .map(|refresh| refresh.carrier_buff_id)
            .unwrap_or(plan.carrier_buff_id);
        Some(BuffFanoutResult {
            rule,
            emitter_uid,
            carrier_buff_uid,
            carrier_buff_id,
            added: Vec::new(),
            refreshed: Vec::new(),
            allies: results,
        })
    }
}
