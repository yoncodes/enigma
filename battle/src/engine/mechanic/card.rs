use sonettobuf::CardInfo;

use crate::engine::{
    manager::{
        BattleManagers,
        eureka::{EurekaState, PowerType},
        ex_point::ExPointKind,
    },
    skill::buff_act::{is_kind, registry::BuffActKind},
    skill::target::{TargetEntity, TargetPool},
};

#[derive(Debug, Clone, Default)]
pub struct CardMechanic;

impl CardMechanic {
    pub(crate) fn uses_action_point(
        &self,
        card: &CardInfo,
        owner_uid: i64,
        skill_id: i32,
        managers: &BattleManagers,
        catalog: &crate::engine::skill::effect::catalog::SkillEffectCatalog,
    ) -> bool {
        (!card.temp_card.unwrap_or_default() || self.is_device_card(managers, card))
            && crate::engine::manager::card::consumes_action_point(card)
            && crate::engine::skill::buff_act::action_point::skill_uses_action_point(
                &managers.buff.active_features(&managers.hp),
                owner_uid,
                catalog.is_big_skill(skill_id),
            )
    }

    pub fn boss_ultimate_power(
        &self,
        managers: &BattleManagers,
        owner_uid: i64,
    ) -> Option<EurekaState> {
        let state = managers
            .eureka
            .get(owner_uid, PowerType::ZongMaoBossEnergy.id());
        (state.max > 0).then_some(state)
    }

    pub fn normal_hand_limit(
        &self,
        base: usize,
        managers: &BattleManagers,
        pool: &TargetPool,
    ) -> usize {
        let modifier = pool
            .attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .map(|entity| {
                managers
                    .buff
                    .buff_act_scalar(entity.uid, BuffActKind::CardLimitAdd)
            })
            .fold(0_i32, i32::saturating_add);
        usize::try_from(
            (base as i32)
                .saturating_add(modifier)
                .saturating_add(managers.card.hand_limit_bonus())
                .max(0),
        )
        .unwrap_or(usize::MAX)
        .max(managers.card.refill_floor())
    }

    pub fn ultimate_cost_offset(&self, managers: &BattleManagers, owner_uid: i64) -> i32 {
        if ExPointKind::from_wire(managers.ex_point.kind(owner_uid)) != ExPointKind::Common {
            return 0;
        }
        managers
            .buff
            .buff_act_scalar(
                owner_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::ExSkillPointChange,
            )
            .saturating_add(managers.buff.buff_act_argument_scalar(
                owner_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::SpExPointMaxAdd,
                1,
            ))
    }

    pub fn required_ultimate_cost(&self, managers: &BattleManagers, entity: &TargetEntity) -> i32 {
        if ExPointKind::from_wire(managers.ex_point.kind(entity.uid)) != ExPointKind::Common {
            return managers
                .catalog()
                .skill_big_skill_point(entity.ex_skill)
                .max(0);
        }
        managers
            .ex_point
            .base_maximum(entity.uid)
            .unwrap_or_else(|| ExPointKind::Common.default_max())
            .saturating_add(self.ultimate_cost_offset(managers, entity.uid))
            .max(0)
    }

    pub fn ultimate_ready(&self, managers: &BattleManagers, entity: &TargetEntity) -> bool {
        if entity.ex_skill <= 0 {
            return false;
        }
        self.ultimate_resource_ready(managers, entity)
    }

    pub fn ultimate_resource_ready(
        &self,
        managers: &BattleManagers,
        entity: &TargetEntity,
    ) -> bool {
        if let Some(power) = self.boss_ultimate_power(managers, entity.uid) {
            return power.is_full()
                && !managers
                    .buff
                    .has_buff_act_kind(entity.uid, BuffActKind::CantGetExskill);
        }
        let kind = ExPointKind::from_wire(managers.ex_point.kind(entity.uid));
        let required = self.required_ultimate_cost(managers, entity);
        let resource_ready = if kind == ExPointKind::Common || required > 0 {
            managers.ex_point.get(entity.uid) >= required
        } else {
            managers.ex_point.is_full(entity.uid)
        };
        resource_ready
            && !managers
                .buff
                .has_buff_act_kind(entity.uid, BuffActKind::CantGetExskill)
    }

    pub fn is_ultimate(
        &self,
        managers: &BattleManagers,
        card: &CardInfo,
        entity: &TargetEntity,
    ) -> bool {
        card.uid == Some(entity.uid)
            && card
                .skill_id
                .is_some_and(|skill_id| self.is_ultimate_skill(managers, skill_id, entity))
    }

    pub(crate) fn is_dealt_ultimate(
        &self,
        managers: &BattleManagers,
        card: &CardInfo,
        entity: &TargetEntity,
    ) -> bool {
        !crate::engine::manager::card::CardManager::is_sovereign(card)
            && self.is_ultimate(managers, card, entity)
    }

    pub fn is_ultimate_skill(
        &self,
        managers: &BattleManagers,
        skill_id: i32,
        entity: &TargetEntity,
    ) -> bool {
        skill_id == entity.ex_skill
            || managers
                .catalog()
                .skill_is_ultimate_for_model(skill_id, entity.model_id)
            || managers
                .ex_point
                .synchronization_definition(entity.uid)
                .is_some_and(|definition| definition.skills.contains(&skill_id))
    }

    pub fn can_add_normal_ultimate(
        &self,
        managers: &BattleManagers,
        entity: &TargetEntity,
    ) -> bool {
        self.ultimate_ready(managers, entity)
            && !self.ultimate_ignores_limit(managers, entity.uid, entity.ex_skill)
            && !managers
                .card
                .hand()
                .iter()
                .chain(managers.card.team_cards())
                .any(|card| self.is_dealt_ultimate(managers, card, entity))
    }

    pub fn refill_hand_len(&self, managers: &BattleManagers, pool: &TargetPool) -> usize {
        managers
            .card
            .hand()
            .iter()
            .filter(|card| self.counts_toward_hand_limit(card, managers, pool))
            .count()
    }

    pub fn counts_toward_hand_limit(
        &self,
        card: &CardInfo,
        managers: &BattleManagers,
        pool: &TargetPool,
    ) -> bool {
        (!card.temp_card.unwrap_or_default() || self.is_device_card(managers, card))
            && pool
                .entity(card.uid.unwrap_or_default())
                .is_none_or(|entity| {
                    !self.skill_ignores_limit(
                        managers,
                        entity.uid,
                        card.skill_id.unwrap_or_default(),
                        self.is_ultimate(managers, card, entity),
                    )
                })
    }

    pub fn is_device_card(&self, managers: &BattleManagers, card: &CardInfo) -> bool {
        card.skill_id.is_some_and(|skill_id| {
            managers.catalog().skill_effect_tag(skill_id)
                == crate::engine::skill::effect::catalog::SkillEffectTag::Device as i32
        })
    }

    pub fn normal_ultimate_cards(
        &self,
        pool: &TargetPool,
        managers: &BattleManagers,
    ) -> Vec<CardInfo> {
        pool.attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .filter(|entity| self.can_add_normal_ultimate(managers, entity))
            .filter_map(|entity| {
                managers
                    .card
                    .draw_pile()
                    .iter()
                    .find(|card| self.is_ultimate(managers, card, entity))
                    .cloned()
                    .or_else(|| {
                        crate::engine::manager::card::pool::card_for_target(entity, entity.ex_skill)
                    })
            })
            .collect()
    }

    pub fn ultimate_ignores_limit(
        &self,
        managers: &BattleManagers,
        owner_uid: i64,
        skill_id: i32,
    ) -> bool {
        self.skill_ignores_limit(managers, owner_uid, skill_id, true)
    }

    fn skill_ignores_limit(
        &self,
        managers: &BattleManagers,
        owner_uid: i64,
        skill_id: i32,
        is_ultimate: bool,
    ) -> bool {
        managers
            .buff
            .active_features(&managers.hp)
            .iter()
            .any(|feature| {
                feature.owner_uid == owner_uid
                    && ((is_kind(feature, BuffActKind::CardNotCalSize)
                        && feature.values.contains(&skill_id))
                        || (is_ultimate && is_kind(feature, BuffActKind::EntityExSkillNotCalSize)))
            })
    }

    pub fn special_team_cards(
        &self,
        pool: &TargetPool,
        managers: &BattleManagers,
        before_hand: &[CardInfo],
    ) -> Vec<CardInfo> {
        pool.attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .filter_map(|entity| {
                let uid = entity.uid;
                let skill_id = entity.ex_skill;
                (self.ultimate_ready(managers, entity)
                    && !before_hand
                        .iter()
                        .any(|card| self.is_dealt_ultimate(managers, card, entity))
                    && self.ultimate_ignores_limit(managers, uid, skill_id))
                .then(|| crate::engine::manager::card::pool::card_for_target(entity, skill_id))
                .flatten()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
