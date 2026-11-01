use sonettobuf::{FightStep, RedealCardInfoPush, UseClothSkillReply, UseClothSkillRequest};

use crate::engine::round::state::continuation_round_shell;

use super::{BattleRuntime, change, drain, executor, project_result, record};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ClothSkillType {
    ClothSkill = 0,
    HeroUpgrade = 1,
    Rouge = 2,
    Contract = 3,
    EzioBigSkill = 4,
    AssassinBigSkill = 5,
    SelectCrystal = 6,
    Rouge2 = 7,
    BattleSelection = 8,
    ExtraRound = 9,
    TwinsSelect = 10,
}

impl TryFrom<i32> for ClothSkillType {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ClothSkill),
            1 => Ok(Self::HeroUpgrade),
            2 => Ok(Self::Rouge),
            3 => Ok(Self::Contract),
            4 => Ok(Self::EzioBigSkill),
            5 => Ok(Self::AssassinBigSkill),
            6 => Ok(Self::SelectCrystal),
            7 => Ok(Self::Rouge2),
            8 => Ok(Self::BattleSelection),
            9 => Ok(Self::ExtraRound),
            10 => Ok(Self::TwinsSelect),
            _ => Err(()),
        }
    }
}

impl BattleRuntime {
    pub fn use_cloth_skill(&mut self, request: UseClothSkillRequest) -> Option<UseClothSkillReply> {
        let mut next = self.clone();
        let reply = next.use_cloth_skill_inner(request)?;
        *self = next;
        Some(reply)
    }

    fn use_cloth_skill_inner(
        &mut self,
        request: UseClothSkillRequest,
    ) -> Option<UseClothSkillReply> {
        self.pending_redeal = None;
        let skill_type = ClothSkillType::try_from(request.r#type?).ok()?;
        let fight_version = self.fight.version.unwrap_or_default();
        let absorb_hurt_map_layout = self.absorb_hurt_map_layout;
        let steps = match skill_type {
            ClothSkillType::ClothSkill => {
                let source_uid = request.from_id.unwrap_or_default();
                let skill_id = request.skill_id?;
                let use_count = self.cloth_skill_uses.get(&skill_id).copied().unwrap_or(0);
                let cloth_id = self.fight.attacker.as_ref()?.cloth_id;
                let (cost, next_cost, cooldown) = self
                    .catalog_data
                    .expect("battle runtime was not constructed with a catalog")
                    .cloth_skill_terms(cloth_id, skill_id, use_count)?;
                let skill_info = self
                    .fight
                    .attacker
                    .as_ref()?
                    .skill_infos
                    .iter()
                    .find(|skill| skill.skill_id == Some(skill_id))?;
                if skill_info.cd.unwrap_or_default() > 0 || self.round_state.power < cost {
                    return None;
                }
                let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
                    self.catalog_data
                        .expect("battle runtime was not constructed with a catalog"),
                    &self.fight,
                )
                .runtime_view(&self.managers);
                self.managers.begin_cloth_sequence();
                let special_steps = match skill_id {
                    30010501 | 30010502 => Some(self.unfinished_tune_steps(
                        skill_id, request.to_id.unwrap_or_default(), &pool,
                        fight_version, absorb_hurt_map_layout,
                    )?),
                    30010701 | 30010702 => Some(self.atonal_sequence_steps(
                        skill_id, request.to_id.unwrap_or_default(),
                        fight_version, absorb_hurt_map_layout,
                    )?),
                    _ => None,
                };
                let result = if special_steps.is_none() { Some(drain::run_skill(
                    &mut self.managers,
                    &pool,
                    &self.catalog,
                    &mut self.determinism,
                    crate::engine::skill::target::TargetContext {
                        battle_id: self.fight.battle_id.unwrap_or_default(),
                        current_round: self.round_state.cur_round,
                        runtime_target_uid: request.to_id.unwrap_or_default(),
                        ..Default::default()
                    },
                    crate::engine::skill::action::SkillRequest {
                        source_uid,
                        skill_id,
                    }
                    .into(),
                    crate::engine::skill::action::SkillModifiers::default(),
                )
                .inspect_err(|error| tracing::warn!(?error, "cloth skill execution failed"))
                .ok()?) } else { None };
                self.round_state.power -= cost;
                if skill_id == 30010502 {
                    let rule = self.catalog_data?.cloth_power(&self.fight)?;
                    self.round_state.power = rule.add(self.round_state.power, 10);
                }
                *self.cloth_skill_uses.entry(skill_id).or_default() += 1;
                self.objectives.record_tuning_use();
                if let Some(attacker) = self.fight.attacker.as_mut() {
                    attacker.power = Some(self.round_state.power);
                    if let Some(skill) = attacker
                        .skill_infos
                        .iter_mut()
                        .find(|skill| skill.skill_id == Some(skill_id))
                    {
                        skill.need_power = Some(next_cost);
                        skill.cd = Some(cooldown);
                    }
                }
                let redeal = result.as_ref().and_then(|result| result.outcomes.iter().find_map(|outcome| {
                    let executor::RuleOutcome::Card(changes) = outcome else {
                        return None;
                    };
                    (changes.kind == crate::engine::manager::card::CardChangeKind::RedealtKeepRanks)
                        .then(|| RedealCardInfoPush {
                            card_group: changes.after.clone(),
                            deal_card_group: Vec::new(),
                        })
                }));
                self.pending_redeal =
                    match crate::engine::fight::versions::redeal_wire_layout(fight_version)? {
                        crate::engine::fight::versions::RedealWireLayout::Version6 => redeal,
                        crate::engine::fight::versions::RedealWireLayout::Version7 => None,
                    };
                if let Some(steps) = special_steps {
                    steps
                } else {
                    project_result(result?, fight_version, absorb_hurt_map_layout)
                        .inspect_err(|error| tracing::warn!(%error, "cloth skill projection failed"))
                        .ok()?
                }
            }
            ClothSkillType::HeroUpgrade => {
                let owner_uid = request.from_id?;
                let upgrade_id = request.skill_id?;
                let option_id = i32::try_from(request.to_id?).ok()?;
                let applied = self.managers.select_upgrade(
                    &mut self.fight,
                    owner_uid,
                    upgrade_id,
                    option_id,
                )?;
                project_changes(
                    [change::BattleChange::UpgradeApplied(Box::new(applied))],
                    fight_version,
                    absorb_hurt_map_layout,
                )
                .inspect_err(|error| tracing::warn!(%error, "hero upgrade projection failed"))
                .ok()?
            }
            ClothSkillType::Contract => {
                if request.skill_id.unwrap_or_default() != 0 {
                    return None;
                }
                let owner_uid = request.from_id?;
                let bound_uid = request.to_id?;
                let origin = self
                    .managers
                    .contract
                    .selection_origin(owner_uid, bound_uid)?;
                let owner = self.managers.entity_snapshot(owner_uid)?;
                let bound = self.managers.entity_snapshot(bound_uid)?;
                let (owner_buff_id, bound_buff_id) =
                    self.managers.catalog().contract_binding_buffs(
                        owner.ex_skill_level.unwrap_or_default(),
                        bound.career?,
                    )?;
                let owner_buff = self
                    .managers
                    .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
                        crate::engine::manager::buff::BuffGrant {
                            origin,
                            source_uid: owner_uid,
                            target_uid: owner_uid,
                            buff_id: owner_buff_id,
                            amount: None,
                            occurrences: 1,
                            child_uid_reservations: 0,
                        },
                    ))
                    .ok()?;
                let owner_selected = self
                    .managers
                    .contract
                    .execute(
                        crate::engine::manager::contract::ContractCommand::SelectOwner {
                            owner_uid,
                            bound_uid,
                        },
                    )
                    .ok()?;
                let bound_buff = self
                    .managers
                    .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
                        crate::engine::manager::buff::BuffGrant {
                            origin,
                            source_uid: owner_uid,
                            target_uid: bound_uid,
                            buff_id: bound_buff_id,
                            amount: None,
                            occurrences: 1,
                            child_uid_reservations: 0,
                        },
                    ))
                    .ok()?;
                let bound_selected = self
                    .managers
                    .contract
                    .execute(
                        crate::engine::manager::contract::ContractCommand::SelectBound {
                            owner_uid,
                            bound_uid,
                        },
                    )
                    .ok()?;
                project_changes(
                    [
                        change::BattleChange::Buff(Box::new(owner_buff)),
                        change::BattleChange::Contract(owner_selected),
                        change::BattleChange::Buff(Box::new(bound_buff)),
                        change::BattleChange::Contract(bound_selected),
                    ],
                    fight_version,
                    absorb_hurt_map_layout,
                )
                .inspect_err(|error| tracing::warn!(%error, "contract selection projection failed"))
                .ok()?
            }
            ClothSkillType::SelectCrystal => {
                let owner_uid = request.from_id?;
                let packed = i32::try_from(request.to_id?).ok()?;
                let selection =
                    crate::engine::mechanic::heat_scale::HeatScale::select_crystals_from_cloth(
                        &mut self.managers,
                        owner_uid,
                        packed,
                    )?;
                project_changes(
                    [
                        change::BattleChange::Buff(Box::new(selection.state)),
                        change::BattleChange::BuffActInfoMarker(selection.marker),
                    ],
                    fight_version,
                    absorb_hurt_map_layout,
                )
                .inspect_err(|error| tracing::warn!(%error, "crystal selection projection failed"))
                .ok()?
            }
            ClothSkillType::TwinsSelect => {
                if request.skill_id.unwrap_or_default() != 0 {
                    return None;
                }
                let owner_uid = request.from_id?;
                let packed = i32::try_from(request.to_id?).ok()?;
                let feature = self
                    .managers
                    .buff
                    .active_features(&self.managers.hp)
                    .into_iter()
                    .find(|feature| {
                        feature.owner_uid == owner_uid
                            && crate::engine::skill::buff_act::is_kind(
                                feature,
                                crate::engine::skill::buff_act::registry::BuffActKind::ConduitCardSelection,
                            )
                    })?;
                let act_id = feature.act_id()?;
                let mut buff = self.managers.buff.snapshot(owner_uid, feature.buff_uid)?;
                if buff.act_info.iter().any(|info| {
                    info.act_id == Some(act_id)
                        && info.param.first().is_some_and(|selected| *selected != 0)
                }) {
                    return None;
                }
                let (skill_ids, params) = crate::engine::skill::buff_act::conduit_select::select(
                    feature.values.get(1..)?,
                    packed,
                )?;
                let origin = crate::engine::skill::buff_act::feature_command_origin(&feature)?;
                let hero_id = self.managers.entity.model_id(owner_uid)?;
                let mut changes = Vec::with_capacity(skill_ids.len() + 2);
                for skill_id in skill_ids {
                    let card = self
                        .managers
                        .execute_card(
                            crate::engine::manager::card::CardCommand::AddSelectedPrecast(
                                crate::engine::manager::card::CardAddPrecast {
                                    origin,
                                    card: crate::engine::manager::card::selected_precast_card(
                                        owner_uid, hero_id, skill_id,
                                    ),
                                },
                            ),
                        )
                        .ok()?;
                    changes.push(change::BattleChange::Card(Box::new(card)));
                }
                if let Some(info) = buff
                    .act_info
                    .iter_mut()
                    .find(|info| info.act_id == Some(act_id))
                {
                    info.param = params.clone();
                } else {
                    buff.act_info.push(sonettobuf::BuffActInfo {
                        act_id: Some(act_id),
                        param: params.clone(),
                        str_param: Some(String::new()),
                    });
                }
                let state = self
                    .managers
                    .execute_buff(crate::engine::manager::buff::BuffCommand::SetInternalState(
                        crate::engine::manager::buff::BuffSetState {
                            origin,
                            target_uid: owner_uid,
                            buff_uid: feature.buff_uid,
                            ex_info: None,
                            params: None,
                            act_info: Some(buff.act_info),
                        },
                    ))
                    .ok()?;
                changes.push(change::BattleChange::Buff(Box::new(state)));
                changes.push(change::BattleChange::BuffActInfoMarker(
                    crate::engine::manager::buff::BuffActInfoMarkerResult {
                        target_uid: owner_uid,
                        buff_uid: feature.buff_uid,
                        act_id,
                        params,
                        str_param: Some(String::new()),
                        team_type: feature.team_type,
                    },
                ));
                let completed = self
                    .managers
                    .execute_buff(
                        crate::engine::manager::buff::BuffCommand::CompleteSelection(
                            crate::engine::manager::buff::BuffRemove {
                                origin,
                                target_uid: owner_uid,
                                selector: crate::engine::manager::buff::BuffRemoveSelector::Uid(
                                    feature.buff_uid,
                                ),
                            },
                        ),
                    )
                    .ok()?;
                changes.push(change::BattleChange::Buff(Box::new(completed)));
                project_changes(changes, fight_version, absorb_hurt_map_layout)
                    .inspect_err(
                        |error| tracing::warn!(%error, "conduit selection projection failed"),
                    )
                    .ok()?
            }
            ClothSkillType::ExtraRound => {
                if request.skill_id.unwrap_or_default() != 0
                    || request.to_id.unwrap_or_default() != 0
                {
                    return None;
                }
                let owner_uid = request.from_id?;
                let granted =
                    crate::engine::mechanic::extra_round::grant(&mut self.managers, owner_uid)?;
                self.round_state.grant_extra_round(crate::engine::skill::action::SkillRequest {
                    source_uid: owner_uid,
                    skill_id: granted.linked_skill,
                });
                let team_type = granted.marker.team_type;
                let frames = [
                    record::SemanticFrame {
                        owner: record::FrameOwner::EventEffect {
                            source_uid: owner_uid,
                            target_uid: 0,
                        },
                        trigger: record::FrameTrigger::Active,
                        items: vec![
                            record::FrameItem::Change(Box::new(change::BattleChange::Buff(
                                Box::new(granted.buff),
                            ))),
                            record::FrameItem::Change(Box::new(
                                change::BattleChange::BuffActInfoMarker(granted.marker),
                            )),
                            record::FrameItem::Cue(record::RoundCue::ExtraRoundGranted),
                        ],
                    },
                    record::SemanticFrame {
                        owner: record::FrameOwner::Command,
                        trigger: record::FrameTrigger::Active,
                        items: vec![
                            record::FrameItem::Cue(record::RoundCue::RedealHandSync {
                                cards: granted.cards,
                            }),
                            record::FrameItem::Cue(record::RoundCue::DeckCount {
                                count: granted.deck_count,
                                team_type,
                            }),
                        ],
                    },
                ];
                crate::engine::packet::timeline::project_for_version_with_absorb_map_layout(
                    &frames,
                    fight_version,
                    absorb_hurt_map_layout,
                )
                .inspect_err(|error| tracing::warn!(?error, "extra round projection failed"))
                .ok()?
            }
            ClothSkillType::EzioBigSkill => {
                let owner_uid = request.from_id?;
                let target_uid = request.to_id?;
                let definition = self
                    .managers
                    .ex_point
                    .synchronization_definition(owner_uid)?;
                let skill_id = match request.skill_id? {
                    1 => definition.skills[0],
                    2 => definition.skills[1],
                    _ => return None,
                };
                let pool = crate::engine::skill::target::TargetPool::from_fight_with_catalog(
                    self.catalog_data
                        .expect("battle runtime was not constructed with a catalog"),
                    &self.fight,
                )
                .runtime_view(&self.managers);
                if !self.managers.buff.has_buff_act_kind(
                    owner_uid,
                    crate::engine::skill::buff_act::registry::BuffActKind::EzioBigSkill,
                ) || !pool
                    .enemies(owner_uid, false)
                    .iter()
                    .any(|target| target.uid == target_uid)
                {
                    return None;
                }
                let progress = self.managers.ex_point.synchronization_progress(owner_uid)?;
                if progress.completed_actions == 0
                    && self.managers.ex_point.get(owner_uid) < definition.threshold
                {
                    return None;
                }
                self.managers.begin_cloth_sequence();
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: owner_uid,
                        skill_id,
                    }
                    .into();
                invocation.target = crate::engine::skill::action::SkillTarget::Explicit(target_uid);
                invocation.mode = crate::engine::skill::action::SkillExecutionMode::Synchronization;
                let result = drain::run_skill(
                    &mut self.managers,
                    &pool,
                    &self.catalog,
                    &mut self.determinism,
                    crate::engine::skill::target::TargetContext {
                        battle_id: self.fight.battle_id.unwrap_or_default(),
                        current_round: self.round_state.cur_round,
                        runtime_target_uid: target_uid,
                        ..Default::default()
                    },
                    invocation,
                    crate::engine::skill::action::SkillModifiers::default(),
                )
                .inspect_err(|error| tracing::warn!(?error, "cloth skill execution failed"))
                .ok()?;
                project_result(result, fight_version, absorb_hurt_map_layout)
                    .inspect_err(|error| tracing::warn!(%error, "cloth skill projection failed"))
                    .ok()?
            }
            _ => return None,
        };
        let mut round = continuation_round_shell(
            &self.fight,
            &self.round_state,
            self.managers.card.ai_queue(),
        );
        self.managers.sync_entities(&mut self.fight);
        round.fight_step = steps;
        self.round = Some(round.clone());
        Some(UseClothSkillReply { round: Some(round) })
    }

    fn atonal_sequence_steps(
        &mut self,
        skill_id: i32,
        selection: i64,
        version: i32,
        layout: crate::engine::fight::versions::AbsorbHurtMapLayout,
    ) -> Option<Vec<FightStep>> {
        use crate::engine::manager::card::{CardAddGenerated, CardAddTemporary, CardCommand, TemporaryCardKind};
        use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};
        let origin = |opcode, name| CommandOrigin { domain: RuleDomain::Behavior,
            key: DefinitionKey::new(opcode, name) };
        let catalog = self.catalog_data?;
        let team = self.fight.attacker.as_ref()?;
        if skill_id == 30010701 {
            let index = usize::try_from(selection.checked_sub(1)?).ok()?;
            let selected = self.managers.card.hand().get(index)?;
            if !crate::engine::mechanic::card::CardMechanic.is_device_card(&self.managers, selected) {
                return None;
            }
            let owner_uid = selected.uid?;
            let owner = team.entitys.iter().find(|hero| hero.uid == Some(owner_uid)
                && self.managers.hp.current(owner_uid) > 0)?;
            let candidates = catalog.device_card_weights(owner).into_iter().flat_map(|(id, count)| {
                std::iter::repeat_n(crate::engine::manager::card::pool::card_for(owner, Some(id)), count).flatten()
            }).collect::<Vec<_>>();
            let replacement = self.determinism.draw_cards(&candidates, 1).into_iter().next()?;
            let removed = self.managers.execute_card(CardCommand::Dissolve {
                origin: origin(30010701, "AtonalDiscardDeviceCard"), card_index: index,
            }).ok()?;
            let added = self.managers.execute_card(CardCommand::AddGenerated(CardAddGenerated {
                origin: origin(30010701, "AtonalDrawDeviceCard"), target_uid: owner_uid,
                skill_id: replacement.skill_id?, hero_id: owner.model_id, team_type: 1,
            })).ok()?;
            return project_changes([
                change::BattleChange::Card(Box::new(removed)),
                change::BattleChange::Card(Box::new(added)),
            ], version, layout).ok();
        }
        if skill_id != 30010702 { return None; }
        let owner = team.entitys.iter().find(|hero| hero.uid.is_some_and(|uid|
            self.managers.hp.current(uid) > 0) && !catalog.device_card_weights(hero).is_empty())?;
        let owner_uid = owner.uid?;
        let generated_skill = self.catalog.get(skill_id)?.slots.iter()
            .find(|slot| slot.behavior.spec.kind == crate::engine::skill::behavior::classify::BehaviorKind::AddSpTempCard2)?
            .behavior.arg(0)?;
        let added = self.managers.execute_card(CardCommand::AddTemporary(CardAddTemporary {
            origin: origin(60300, "AddSpTempCard2"), target_uid: owner_uid,
            skill_id: generated_skill, hero_id: owner.model_id, reserve_id: i64::from(owner.model_id?),
            team_type: 1, kind: TemporaryCardKind::ConfiguredSkill,
        })).ok()?;
        let card = added.added.clone()?;
        let mut steps = project_changes([change::BattleChange::Card(Box::new(added))], version, layout).ok()?;
        let step = steps.first_mut()?;
        let mut effect = crate::engine::packet::card::CardPacket::add_hand_card(owner_uid, card);
        effect.effect_type = Some(sonettobuf::effect_type_enum::EffectType::Addsphandcard as i32);
        effect.effect_num1 = Some(skill_id);
        effect.team_type = Some(1);
        step.act_effect.clear();
        step.act_effect.push(effect);
        Some(steps)
    }

    fn unfinished_tune_steps(
        &mut self,
        skill_id: i32,
        target_uid: i64,
        pool: &crate::engine::skill::target::TargetPool,
        version: i32,
        layout: crate::engine::fight::versions::AbsorbHurtMapLayout,
    ) -> Option<Vec<FightStep>> {
        use crate::engine::manager::card::{CardAddPrecast, CardCommand, selected_precast_card};
        use crate::engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain};
        let origin = |id, name| CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(id, name),
        };
        let mut changes = Vec::new();
        match skill_id {
            30010501 => {
                let target = pool.attacker_main.iter()
                    .find(|target| target.uid == target_uid && target.current_hp > 0)?;
                let catalog = self.catalog_data?;
                let first_rank = |group: &[i32]| group.iter().copied()
                    .find(|id| catalog.skill_rank(*id) == 1);
                let ids = [first_rank(&target.skill_group1)?, first_rank(&target.skill_group2)?];
                for id in ids {
                    let added = self.managers.execute_card(CardCommand::AddSelectedPrecast(
                        CardAddPrecast {
                            origin: origin(60032, "AddSkillCard"),
                            card: selected_precast_card(target_uid, target.model_id, id),
                        },
                    )).ok()?;
                    changes.push(change::BattleChange::Card(Box::new(added)));
                }
            }
            30010502 => {
                let catalog = self.catalog_data?;
                // The client renders the first internal card at the right edge.
                let index = self.managers.card.hand().iter().position(|card| {
                    let skill = card.skill_id.unwrap_or_default();
                    let model = card.hero_id.unwrap_or_default();
                    let owner = pool.entity(card.uid.unwrap_or_default());
                    skill > 0 && !self.catalog.is_big_skill(skill)
                        && !catalog.skill_is_ultimate_for_model(skill, model)
                        && !owner.is_some_and(|owner| owner.ex_skill == skill)
                });
                if let Some(card_index) = index {
                    let removed = self.managers.execute_card(CardCommand::Dissolve {
                        origin: origin(60031, "RemoveRightCard"), card_index,
                    }).ok()?;
                    changes.push(change::BattleChange::Card(Box::new(removed)));
                }
            }
            _ => return None,
        }
        project_changes(changes, version, layout).ok()
    }

    pub fn take_redeal_card_push(&mut self) -> Option<RedealCardInfoPush> {
        self.pending_redeal.take()
    }
}

fn project_changes(
    changes: impl IntoIterator<Item = change::BattleChange>,
    fight_version: i32,
    absorb_hurt_map_layout: crate::engine::fight::versions::AbsorbHurtMapLayout,
) -> Result<Vec<FightStep>, String> {
    let frame = record::SemanticFrame {
        owner: record::FrameOwner::Command,
        trigger: record::FrameTrigger::Active,
        items: changes
            .into_iter()
            .map(|change| record::FrameItem::Change(Box::new(change)))
            .collect(),
    };
    crate::engine::packet::timeline::project_for_version_with_absorb_map_layout(
        &[frame],
        fight_version,
        absorb_hurt_map_layout,
    )
    .map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod tests;
