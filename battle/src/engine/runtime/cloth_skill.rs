use sonettobuf::{Fight, FightStep, RedealCardInfoPush, UseClothSkillReply, UseClothSkillRequest};

use crate::engine::round::state::next_round_shell;

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
        let steps = match skill_type {
            ClothSkillType::ClothSkill => {
                let source_uid = request.from_id.unwrap_or_default();
                let skill_id = request.skill_id?;
                let use_count = self.cloth_skill_uses.get(&skill_id).copied().unwrap_or(0);
                let (cost, next_cost, cooldown) =
                    cloth_skill_terms(&self.fight, skill_id, use_count)?;
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
                let pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight)
                    .runtime_view(&self.managers);
                let result = drain::run_skill(
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
                .ok()?;
                self.round_state.power -= cost;
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
                let redeal = result.outcomes.iter().find_map(|outcome| {
                    let executor::RuleOutcome::Card(changes) = outcome else {
                        return None;
                    };
                    (changes.kind == crate::engine::manager::card::CardChangeKind::RedealtKeepRanks)
                        .then(|| RedealCardInfoPush {
                            card_group: changes.after.clone(),
                            deal_card_group: Vec::new(),
                        })
                });
                self.pending_redeal =
                    match crate::engine::fight::versions::redeal_wire_layout(fight_version)? {
                        crate::engine::fight::versions::RedealWireLayout::Version6 => redeal,
                        crate::engine::fight::versions::RedealWireLayout::Version7 => None,
                    };
                project_result(result, fight_version)
                    .inspect_err(|error| tracing::warn!(%error, "cloth skill projection failed"))
                    .ok()?
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
                )
                .inspect_err(|error| tracing::warn!(%error, "hero upgrade projection failed"))
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
                project_changes(changes, fight_version)
                    .inspect_err(
                        |error| tracing::warn!(%error, "conduit selection projection failed"),
                    )
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
                let pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight)
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
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: owner_uid,
                        skill_id,
                    }
                    .into();
                invocation.target = crate::engine::skill::action::SkillTarget::Explicit(target_uid);
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
                project_result(result, fight_version)
                    .inspect_err(|error| tracing::warn!(%error, "cloth skill projection failed"))
                    .ok()?
            }
            _ => return None,
        };
        let mut round = next_round_shell(
            &self.fight,
            &self.round_state,
            !self.round_state.is_finish,
            self.managers.card.hand(),
            self.managers.card.team_cards(),
            self.managers.card.ai_queue(),
        );
        self.managers.sync_entities(&mut self.fight);
        round.fight_step = steps;
        self.round = Some(round.clone());
        Some(UseClothSkillReply { round: Some(round) })
    }

    pub fn take_redeal_card_push(&mut self) -> Option<RedealCardInfoPush> {
        self.pending_redeal.take()
    }
}

fn cloth_skill_terms(fight: &Fight, skill_id: i32, use_count: usize) -> Option<(i32, i32, i32)> {
    let cloth_id = fight.attacker.as_ref()?.cloth_id.unwrap_or(1);
    let cloth = config::configs::get()
        .cloth_level
        .iter()
        .find(|cloth| cloth.id == cloth_id && cloth.level == 1)?;
    let (costs, cooldown) = if cloth.skill1 == skill_id {
        (&cloth.use_power1, cloth.cd1)
    } else if cloth.skill2 == skill_id {
        (&cloth.use_power2, cloth.cd2)
    } else {
        return None;
    };
    let cost = *costs.get(use_count.min(costs.len().checked_sub(1)?))?;
    let next = *costs
        .get((use_count + 1).min(costs.len().saturating_sub(1)))
        .unwrap_or(&cost);
    Some((cost, next, cooldown))
}

fn project_changes(
    changes: impl IntoIterator<Item = change::BattleChange>,
    fight_version: i32,
) -> Result<Vec<FightStep>, String> {
    let frame = record::SemanticFrame {
        owner: record::FrameOwner::Command,
        trigger: record::FrameTrigger::Active,
        items: changes
            .into_iter()
            .map(|change| record::FrameItem::Change(Box::new(change)))
            .collect(),
    };
    crate::engine::packet::timeline::project_for_version(&[frame], fight_version)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_match_lua_fight_enum() {
        assert_eq!(ClothSkillType::try_from(1), Ok(ClothSkillType::HeroUpgrade));
        assert_eq!(ClothSkillType::try_from(2), Ok(ClothSkillType::Rouge));
        assert_eq!(
            ClothSkillType::try_from(8),
            Ok(ClothSkillType::BattleSelection)
        );
        assert!(ClothSkillType::try_from(9).is_err());
        assert_eq!(
            ClothSkillType::try_from(10),
            Ok(ClothSkillType::TwinsSelect)
        );
    }
}
