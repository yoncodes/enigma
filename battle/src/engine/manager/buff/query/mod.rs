use super::update::special_count_value;
use super::*;
use crate::engine::skill::buff_act::registry::BuffActKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialCountOutput {
    pub source_uid: i64,
    pub marker_buff_id: i32,
    pub target_uid: i64,
    pub output_buff_id: i32,
    pub output_act_id: i32,
    pub amount: i32,
}

impl BuffManager {
    pub fn buff_act_state(&self, owner_uid: i64, kind: BuffActKind) -> Option<(i32, &str)> {
        self.buffs.iter().find_map(|active| {
            if active.owner_uid != owner_uid {
                return None;
            }
            let act_id = active
                .definition
                .as_ref()?
                .features()
                .iter()
                .find(|feature| feature.kind == Some(kind))?
                .values
                .first()
                .copied()?;
            Some((act_id, active.buff.act_common_params.as_deref()?))
        })
    }

    pub fn act_common_value(&self, owner_uid: i64, buff_uid: i64, act_id: i32) -> Option<i32> {
        let raw = self
            .buffs
            .iter()
            .find(|active| active.owner_uid == owner_uid && active.buff.uid == Some(buff_uid))?
            .buff
            .act_common_params
            .as_deref()?;
        let (actual_act_id, value) = raw.split_once('#')?;
        (actual_act_id.parse::<i32>().ok()? == act_id)
            .then(|| value.parse::<i32>().ok())
            .flatten()
    }

    pub fn layer_halo_sync(&self) -> Vec<super::BuffSyncResult> {
        self.entities
            .iter()
            .flat_map(|entity| {
                self.buffs
                    .iter()
                    .filter(move |active| active.owner_uid == entity.uid)
                    .filter(|active| super::is_layer_halo_slave(&active.buff))
                    .filter(|active| {
                        active.definition.as_ref().is_some_and(|definition| {
                            definition.features().iter().any(|feature| {
                                feature.kind
                                    == Some(
                                        crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo,
                                    )
                            })
                        })
                    })
                    .map(|active| super::BuffSyncResult {
                        target_uid: active.owner_uid,
                        buff: active.buff.clone(),
                    })
            })
            .collect()
    }

    pub(crate) fn has_status(&self, owner_uid: i64, status: BuffStatus) -> bool {
        self.buffs.iter().any(|active| {
            active.owner_uid == owner_uid
                && active
                    .definition
                    .as_ref()
                    .is_some_and(|definition| definition.status == status)
        })
    }

    pub(crate) fn buffs_with_status(&self, owner_uid: i64, status: BuffStatus) -> Vec<(i32, i32)> {
        self.buffs
            .iter()
            .filter(|active| {
                active.owner_uid == owner_uid
                    && active
                        .definition
                        .as_ref()
                        .is_some_and(|definition| definition.status == status)
            })
            .filter_map(|active| Some((active.buff.buff_id?, super::count_or_layer(&active.buff))))
            .collect()
    }

    pub fn buff_act_carrier(&self, owner_uid: i64, kind: BuffActKind) -> Option<(i64, i32, i32)> {
        self.buffs.iter().find_map(|active| {
            let feature = active
                .definition
                .as_ref()?
                .features()
                .iter()
                .find(|feature| feature.kind == Some(kind))?;
            Some((active.buff.uid?, *feature.values.first()?, active.team_type))
                .filter(|_| active.owner_uid == owner_uid)
        })
    }

    pub fn has_buff_act_kind(&self, owner_uid: i64, kind: BuffActKind) -> bool {
        self.buffs.iter().any(|active| {
            active.owner_uid == owner_uid
                && active.definition.as_ref().is_some_and(|definition| {
                    definition
                        .features()
                        .iter()
                        .any(|feature| feature.kind == Some(kind))
                })
        })
    }

    pub fn buff_act_scalar(&self, owner_uid: i64, kind: BuffActKind) -> i32 {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == owner_uid)
            .filter_map(|active| {
                let amount = count_or_layer(&active.buff).max(1);
                active.definition.as_ref().map(|definition| {
                    definition
                        .features()
                        .iter()
                        .filter(|feature| feature.kind == Some(kind))
                        .filter_map(|feature| feature.values.get(1))
                        .map(|value| value.saturating_mul(amount))
                        .sum::<i32>()
                })
            })
            .sum()
    }

    pub fn buff_act_amount(&self, owner_uid: i64, kind: BuffActKind) -> i32 {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == owner_uid)
            .filter(|active| {
                active.definition.as_ref().is_some_and(|definition| {
                    definition
                        .features()
                        .iter()
                        .any(|feature| feature.kind == Some(kind))
                })
            })
            .map(|active| count_or_layer(&active.buff).max(1))
            .sum()
    }

    pub fn buff_act_value(&self, owner_uid: i64, kind: BuffActKind) -> i32 {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == owner_uid)
            .filter_map(|active| {
                let buff_uid = active.buff.uid?;
                active
                    .definition
                    .as_ref()?
                    .features()
                    .iter()
                    .find_map(|feature| {
                        (feature.kind == Some(kind)).then(|| {
                            feature
                                .values
                                .first()
                                .map(|act_id| self.act_value(buff_uid, *act_id))
                                .unwrap_or_default()
                        })
                    })
            })
            .sum()
    }

    pub fn duration_stage(&self, owner_uid: i64, buff_uid: i64) -> Option<i32> {
        self.buffs
            .iter()
            .find(|active| active.owner_uid == owner_uid && active.buff.uid == Some(buff_uid))?
            .definition
            .as_ref()
            .map(|definition| definition.take_stage)
    }

    pub fn lost_life_floor_permille(&self, owner_uid: i64) -> i32 {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == owner_uid)
            .filter_map(|active| active.definition.as_ref())
            .flat_map(BuffDefinition::features)
            .filter(|feature| {
                feature.kind
                    == Some(crate::engine::skill::buff_act::registry::BuffActKind::BanLostLife)
            })
            .filter_map(|feature| feature.values.get(1).copied())
            .max()
            .unwrap_or_default()
            .max(0)
    }

    pub fn special_count_outputs(&self, hp: &HpManager) -> Vec<SpecialCountOutput> {
        let mut outputs = Vec::new();
        for marker in &self.buffs {
            let count = special_count_value(marker.buff.act_common_params.as_deref());
            if marker.owner_uid == 0 || count <= 0 {
                continue;
            }
            let Some(channel) = marker
                .definition
                .as_ref()
                .and_then(|definition| {
                    definition
                        .features()
                        .iter()
                        .find(|feature| {
                            feature.values.first().is_some_and(|act_id| {
                                crate::engine::skill::buff_act::registry::kind(
                                    *act_id,
                                    &feature.act_type,
                                ) == Some(
                                    crate::engine::skill::buff_act::registry::BuffActKind::SpecialCountContinueChannelBuff,
                                )
                            })
                        })
                })
                .map(|feature| feature.values.as_slice())
            else {
                continue;
            };
            let [_, target_lane, output_buff_id, _] = channel else {
                continue;
            };
            let Some(output_definition) = BuffDefinition::get(*output_buff_id) else {
                continue;
            };
            let Some(output) = output_definition
                .features()
                .iter()
                .find(|feature| {
                    feature.values.first().is_some_and(|act_id| {
                        crate::engine::skill::buff_act::registry::kind(
                            *act_id,
                            &feature.act_type,
                        ) == Some(
                            crate::engine::skill::buff_act::registry::BuffActKind::AddAttrBySpecialCount,
                        )
                    })
                })
            else {
                continue;
            };
            let [output_act_id, _, base, per_count, marker_ids @ ..] = output.values.as_slice()
            else {
                continue;
            };
            let amount = *base + *per_count * self.special_count(marker.owner_uid, marker_ids);
            let team_type = marker.team_type;
            for target in self.entities.iter().filter(|target| {
                hp.current(target.uid) > 0
                    && match target_lane {
                        1 => target.team_type == team_type,
                        2 => target.team_type != team_type,
                        103 | 309 => target.uid == marker.owner_uid,
                        _ => false,
                    }
            }) {
                if self.buffs.iter().any(|active| {
                    active.owner_uid == target.uid
                        && active.buff.buff_id == Some(*output_buff_id)
                        && active.buff.from_uid == Some(marker.owner_uid)
                }) {
                    continue;
                }
                outputs.push(SpecialCountOutput {
                    source_uid: marker.owner_uid,
                    marker_buff_id: marker.buff.buff_id.unwrap_or_default(),
                    target_uid: target.uid,
                    output_buff_id: *output_buff_id,
                    output_act_id: *output_act_id,
                    amount,
                });
            }
        }
        outputs
    }

    pub fn special_count(&self, owner_uid: i64, marker_ids: &[i32]) -> i32 {
        self.buffs
            .iter()
            .filter(|active| {
                active.owner_uid == owner_uid
                    && marker_ids
                        .iter()
                        .any(|id| *id == active.type_id || active.buff.buff_id == Some(*id))
            })
            .map(|active| special_count_value(active.buff.act_common_params.as_deref()))
            .sum()
    }

    pub fn active_for(&self, uid: i64) -> impl Iterator<Item = &BuffInfo> {
        self.buffs
            .iter()
            .filter_map(move |active| (active.owner_uid == uid).then_some(&active.buff))
    }

    pub fn has_buff_id(&self, uid: i64, buff_id: i32) -> bool {
        self.active_for(uid)
            .any(|buff| buff.buff_id == Some(buff_id))
    }

    pub(crate) fn buff_family_carrier_uid(&self, uid: i64, buff_id: i32) -> Option<i64> {
        let incoming = BuffDefinition::get(buff_id);
        self.buffs
            .iter()
            .find(|active| {
                active.owner_uid == uid
                    && incoming.as_ref().map_or_else(
                        || active.buff.buff_id == Some(buff_id),
                        |incoming| {
                            if incoming.uses_shared_type_family() {
                                active.type_id == incoming.effective_type_id()
                                    && active
                                        .definition
                                        .as_ref()
                                        .is_some_and(BuffDefinition::uses_shared_type_family)
                            } else {
                                active.buff.buff_id == Some(buff_id)
                            }
                        },
                    )
            })
            .and_then(|active| active.buff.uid)
    }

    pub(crate) fn shield_carrier_uid(&self, owner_uid: i64) -> Option<i64> {
        self.buffs.iter().find_map(|active| {
            (active.owner_uid == owner_uid
                && active
                    .definition
                    .as_ref()
                    .is_some_and(|definition| definition.status == BuffStatus::Shield))
            .then_some(active.buff.uid)
            .flatten()
        })
    }

    pub fn has_buff_id_or_type(&self, uid: i64, buff_id_or_type_id: i32) -> bool {
        self.buffs.iter().any(|active| {
            active.owner_uid == uid
                && (active.buff.buff_id == Some(buff_id_or_type_id)
                    || active.type_id == buff_id_or_type_id)
        })
    }

    pub fn has_active_buff_id_or_type(&self, uid: i64, buff_id_or_type_id: i32) -> bool {
        self.buffs.iter().any(|active| {
            active.owner_uid == uid
                && (active.definition.as_ref().is_none_or(|definition| {
                    definition.duration <= 0 || active.buff.duration.unwrap_or_default() > 0
                }))
                && (active.buff.buff_id == Some(buff_id_or_type_id)
                    || active.type_id == buff_id_or_type_id)
        })
    }

    pub fn buff_id_amount(&self, uid: i64, buff_id: i32) -> i32 {
        self.active_for(uid)
            .filter(|buff| buff.buff_id == Some(buff_id))
            .map(count_or_layer)
            .sum()
    }

    pub fn buff_id_or_type_amount(&self, uid: i64, buff_id_or_type_id: i32) -> i32 {
        self.buffs
            .iter()
            .filter(|active| {
                active.owner_uid == uid
                    && (active.buff.buff_id == Some(buff_id_or_type_id)
                        || active.type_id == buff_id_or_type_id)
            })
            .map(|active| count_or_layer(&active.buff))
            .sum()
    }

    pub fn buff_id_or_type_uid(&self, uid: i64, buff_id_or_type_id: i32) -> Option<i64> {
        self.buffs
            .iter()
            .find(|active| {
                active.owner_uid == uid
                    && (active.buff.buff_id == Some(buff_id_or_type_id)
                        || active.type_id == buff_id_or_type_id)
            })
            .and_then(|active| active.buff.uid)
    }

    pub fn buff_id_uid(&self, uid: i64, buff_id: i32) -> Option<i64> {
        self.buffs
            .iter()
            .find(|active| active.owner_uid == uid && active.buff.buff_id == Some(buff_id))
            .and_then(|active| active.buff.uid)
    }

    pub fn stack_limit(&self, buff_id: i32) -> i32 {
        BuffDefinition::get(buff_id)
            .map(|definition| definition.stack_max_layer())
            .unwrap_or_default()
    }

    pub fn grant_overflow(
        &self,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        amount: i32,
    ) -> i32 {
        let Some(definition) = BuffDefinition::get(buff_id) else {
            return 0;
        };
        let limit =
            self.grant_stack_limit(BuffRoute::new(source_uid, target_uid, buff_id), &definition);
        if limit <= 0 {
            return 0;
        }
        self.buff_id_amount(target_uid, buff_id)
            .saturating_add(amount.max(0))
            .saturating_sub(limit)
    }

    pub fn buff_type_amount(&self, uid: i64, type_id: i32) -> i32 {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == uid && active.type_id == type_id)
            .map(|active| count_or_layer(&active.buff))
            .sum()
    }

    pub fn buff_group_type_count(&self, uid: i64, group_ids: &[i32]) -> i32 {
        let mut type_ids = Vec::new();
        for active in self.buffs.iter().filter(|active| {
            active.owner_uid == uid
                && active
                    .definition
                    .as_ref()
                    .is_some_and(|definition| group_ids.contains(&definition.group))
        }) {
            if !type_ids.contains(&active.type_id) {
                type_ids.push(active.type_id);
            }
        }
        type_ids.len() as i32
    }

    pub fn buff_group_amount(&self, uid: i64, group_id: i32) -> i32 {
        self.buffs
            .iter()
            .filter(|active| {
                active.owner_uid == uid
                    && active
                        .definition
                        .as_ref()
                        .is_some_and(|definition| definition.group == group_id)
            })
            .map(|active| count_or_layer(&active.buff))
            .sum()
    }

    pub fn buff_status_count(&self, uid: i64, status_ids: &[i32]) -> i32 {
        self.buffs
            .iter()
            .filter(|active| {
                active.owner_uid == uid
                    && active.definition.as_ref().is_some_and(|definition| {
                        status_ids
                            .iter()
                            .any(|status_id| definition.matches_type_category(*status_id))
                    })
            })
            .count() as i32
    }

    pub fn team_buff_status_type_count(&self, uids: &[i64], status_ids: &[i32]) -> i32 {
        let mut type_ids = Vec::new();
        for active in self.buffs.iter().filter(|active| {
            uids.contains(&active.owner_uid)
                && active.definition.as_ref().is_some_and(|definition| {
                    status_ids
                        .iter()
                        .any(|status_id| definition.matches_type_category(*status_id))
                })
        }) {
            if !type_ids.contains(&active.type_id) {
                type_ids.push(active.type_id);
            }
        }
        type_ids.len() as i32
    }

    pub fn buff_type_layer(&self, uid: i64, type_id: i32) -> i32 {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == uid && active.type_id == type_id)
            .map(|active| active.buff.layer.unwrap_or_default())
            .sum()
    }

    pub fn max_id_or_type_layer(&self, uid: i64, id: i32) -> i32 {
        self.buffs
            .iter()
            .filter(|active| {
                active.owner_uid == uid && (active.type_id == id || active.buff.buff_id == Some(id))
            })
            .map(|active| active.buff.layer.unwrap_or_default())
            .max()
            .unwrap_or_default()
    }

    pub fn attribute_delta(&self, uid: i64, attr_id: AttrId) -> i32 {
        let team_type = self.team_type(uid);
        let deltas = || {
            let own = self
                .buffs
                .iter()
                .filter(|active| active.owner_uid == uid)
                .filter_map(|active| {
                    let definition = active.definition.as_ref()?;
                    let delta = definition
                        .attribute_deltas()
                        .iter()
                        .filter_map(|(actual, value)| (*actual == attr_id).then_some(*value))
                        .sum::<i32>()
                        * attribute_amount(&active.buff)
                        + definition
                            .features()
                            .iter()
                            .filter(|feature| {
                                feature.kind == Some(BuffActKind::AttrAndLayerAttr)
                            })
                            .map(|feature| {
                                crate::engine::skill::buff_act::attr_and_layer_attr::attribute_delta(
                                    &feature.values,
                                    active.buff.layer.unwrap_or_default(),
                                    attr_id,
                                )
                            })
                            .sum::<i32>();
                    (delta != 0).then_some((active.buff.buff_id.unwrap_or_default(), delta))
                });
            let team = self
                .buffs
                .iter()
                .filter(|active| Some(active.team_type) == team_type)
                .filter_map(|active| {
                    let buff_uid = active.buff.uid?;
                    let delta = active
                        .definition
                        .as_ref()?
                        .features()
                        .iter()
                        .filter(|feature| {
                            feature.kind
                                == Some(BuffActKind::TeamExElectricTransConsumeValueAttr)
                        })
                        .filter_map(|feature| {
                            let [act_id, args @ ..] = feature.values.as_slice() else {
                                return None;
                            };
                            let rule =
                                crate::engine::skill::buff_act::electric_transform::team_attribute_rule(
                                    args,
                                )?;
                            let dynamic = if rule.dynamic_attr == attr_id {
                                self.grant_value(buff_uid, *act_id).unwrap_or_default()
                            } else {
                                0
                            };
                            let fixed = if rule.fixed_attr == attr_id {
                                rule.fixed_value
                            } else {
                                0
                            };
                            Some(dynamic.saturating_add(fixed))
                        })
                        .sum::<i32>()
                        * attribute_amount(&active.buff);
                    (delta != 0).then_some((active.buff.buff_id.unwrap_or_default(), delta))
                });
            own.chain(team)
        };
        if attr_id == AttrId::DmgBonus
            && crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage)
        {
            let parts = deltas().collect::<Vec<_>>();
            if !parts.is_empty() {
                eprintln!("buff attribute source={uid} attr={attr_id:?} parts={parts:?}");
            }
            return parts.into_iter().map(|(_, delta)| delta).sum();
        }
        deltas().map(|(_, delta)| delta).sum()
    }

    pub fn configured_attribute_deltas(buff_id: i32) -> Vec<(AttrId, i32)> {
        BuffDefinition::get(buff_id)
            .map(|definition| definition.attribute_deltas().to_vec())
            .unwrap_or_default()
    }

    pub fn configured_accepts_explicit_grant_amount(buff_id: i32) -> bool {
        BuffDefinition::get(buff_id)
            .is_some_and(|definition| definition.accepts_explicit_grant_amount())
    }

    pub fn has_buff_type(&self, uid: i64, type_id: i32) -> bool {
        self.has_buff_id_or_type(uid, type_id)
    }

    pub fn team_type(&self, uid: i64) -> Option<i32> {
        match uid {
            crate::engine::fight::rules::ATTACKER_SIDE_UID => Some(1),
            crate::engine::fight::rules::DEFENDER_SIDE_UID => Some(2),
            _ => self.team_types.get(&uid).copied(),
        }
    }

    pub fn alive_team_types(&self, hp: &HpManager) -> Vec<i32> {
        let mut teams = Vec::new();
        for entity in &self.entities {
            if hp.current(entity.uid) > 0 && !teams.contains(&entity.team_type) {
                teams.push(entity.team_type);
            }
        }
        teams
    }

    pub fn alive_team_uids(&self, team_type: i32, hp: &HpManager) -> Vec<i64> {
        self.entities
            .iter()
            .filter(|entity| entity.team_type == team_type && hp.current(entity.uid) > 0)
            .map(|entity| entity.uid)
            .collect()
    }

    pub fn active_features(&self, hp: &HpManager) -> Vec<ActiveBuffFeature> {
        let materialized = self
            .buffs
            .iter()
            .filter_map(|active| {
                Some((
                    active.owner_uid,
                    active.buff.from_uid.unwrap_or_default(),
                    active.buff.buff_id?,
                ))
            })
            .collect::<std::collections::HashSet<_>>();
        self.buffs
            .iter()
            .flat_map(|active| {
                let owner_alive = hp.current(active.owner_uid) > 0;
                let owner_uid = active.owner_uid;
                let source_uid = active.buff.from_uid.unwrap_or_default();
                let root_buff_id = active.buff.buff_id.unwrap_or_default();
                let materialized = &materialized;
                active_feature(
                    owner_uid,
                    active.team_type,
                    owner_alive,
                    &active.buff,
                    active.definition.as_ref(),
                )
                .into_iter()
                .filter(move |feature| {
                    feature.buff_id == root_buff_id
                        || !materialized.contains(&(owner_uid, source_uid, feature.buff_id))
                })
            })
            .collect()
    }

    pub fn detonation_features(
        &self,
        hp: &HpManager,
        owner_uid: i64,
        selectors: &[i32],
        include_poison: bool,
    ) -> Vec<ActiveBuffFeature> {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == owner_uid)
            .flat_map(|active| {
                let selected = selectors.iter().any(|selector| {
                    *selector == active.type_id || active.buff.buff_id == Some(*selector)
                });
                active_feature(
                    active.owner_uid,
                    active.team_type,
                    hp.current(active.owner_uid) > 0,
                    &active.buff,
                    active.definition.as_ref(),
                )
                .into_iter()
                .filter(move |feature| {
                    selected
                        || (include_poison
                            && crate::engine::skill::buff_act::is_kind(
                                feature,
                                crate::engine::skill::buff_act::registry::BuffActKind::Poison,
                            ))
                })
            })
            .collect()
    }

    pub fn configured_features(buff_id: i32) -> Vec<ActiveBuffFeature> {
        let definition = BuffDefinition::get(buff_id);
        active_feature(
            0,
            0,
            true,
            &BuffInfo {
                buff_id: Some(buff_id),
                count: Some(1),
                layer: Some(1),
                ..Default::default()
            },
            definition.as_ref(),
        )
    }

    pub fn passive_skill_links_for(&self, uid: i64) -> Vec<BuffPassiveSkillLink> {
        self.buffs
            .iter()
            .filter(|active| active.owner_uid == uid)
            .flat_map(|active| {
                let owner_uid = passive_skill_owner(active);
                let amount = count_or_layer(&active.buff);
                active
                    .definition
                    .as_ref()
                    .map(|definition| passive_skill_links(owner_uid, definition.features(), amount))
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn static_power_max_adds(&self) -> Vec<BuffPowerMaxAdd> {
        self.buffs
            .iter()
            .flat_map(|active| {
                let amount = count_or_layer(&active.buff);
                active
                    .definition
                    .as_ref()
                    .map(|definition| {
                        power_max_add(
                            active.owner_uid,
                            &active.buff,
                            definition.features(),
                            amount,
                        )
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn static_hp_max_add_rates(&self) -> Vec<BuffHpMaxAddRate> {
        self.buffs
            .iter()
            .flat_map(|active| {
                let amount = count_or_layer(&active.buff);
                active
                    .definition
                    .as_ref()
                    .map(|definition| {
                        hp_max_add_rate(
                            active.owner_uid,
                            &active.buff,
                            definition.features(),
                            amount,
                        )
                    })
                    .unwrap_or_default()
            })
            .collect()
    }
}

fn passive_skill_owner(active: &ActiveBuff) -> i64 {
    let source_owned = active.definition.as_ref().is_some_and(|definition| {
        definition
            .features()
            .iter()
            .any(|feature| feature.kind == Some(BuffActKind::SlaveHalo))
    });
    if source_owned {
        active
            .buff
            .from_uid
            .filter(|source_uid| *source_uid != 0)
            .unwrap_or(active.owner_uid)
    } else {
        active.owner_uid
    }
}

#[cfg(test)]
mod tests;
