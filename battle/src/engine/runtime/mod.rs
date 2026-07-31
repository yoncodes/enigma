use std::collections::HashMap;

use sonettobuf::{
    CardInfo, Fight, FightRound, FightStatistics, HeroExAttribute, HeroSpAttribute,
    RedealCardInfoPush, UseCardStatistics,
};

use crate::engine::{
    manager::BattleManagers,
    round::{outcome::battle_outcome, state::RoundState},
    skill::effect::SkillEffectCatalog,
};

use self::determinism::RoundDeterminism;

pub use crate::engine::round::outcome::BattleOutcome;

pub(crate) mod change;
pub mod cloth_skill;
pub mod determinism;
pub(crate) mod drain;
pub(crate) mod executor;
pub(crate) mod record;
mod round;
pub(crate) mod schedule;
pub mod skill;
mod start;

pub use self::cloth_skill::ClothSkillType;

/// Owns one authoritative battle simulation and its deterministic round state.
///
/// Durable combat state lives in `managers`; protobuf `Fight` is synchronized
/// only at explicit response boundaries.
#[derive(Debug, Clone, Default)]
pub struct BattleRuntime {
    fight: Fight,
    managers: BattleManagers,
    catalog: SkillEffectCatalog,
    round: Option<FightRound>,
    round_state: RoundState,
    determinism: RoundDeterminism,
    pending_redeal: Option<RedealCardInfoPush>,
    cloth_skill_uses: HashMap<i32, usize>,
}

impl BattleRuntime {
    /// Plans client auto-battle operations without mutating authoritative state.
    pub fn plan_auto_round(
        &self,
        request: &sonettobuf::AutoRoundRequest,
    ) -> sonettobuf::AutoRoundReply {
        crate::engine::auto_battle::plan(
            request,
            &self.fight,
            &self.managers,
            &self.catalog,
            &self.round_state,
            &self.determinism,
            self.conduit_operations(),
        )
    }

    /// Evaluates the current terminal or wave outcome from manager-owned state.
    pub fn outcome(&self) -> BattleOutcome {
        let pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
        battle_outcome(&self.fight, &pool, &self.managers)
    }

    pub fn current_round(&self) -> i32 {
        self.round_state.cur_round
    }

    pub fn fight_version(&self) -> i32 {
        self.fight.version.unwrap_or_default()
    }

    /// Returns the synchronized fight state and most recently committed round for reconnect.
    pub fn reconnect_state(&self) -> (Fight, Option<FightRound>) {
        (self.fight.clone(), self.round.clone())
    }

    pub fn dead_attacker_count(&self) -> usize {
        self.fight
            .attacker
            .iter()
            .flat_map(|team| team.entitys.iter())
            .filter_map(|entity| entity.uid)
            .filter(|uid| self.managers.hp.current(*uid) <= 0)
            .count()
    }

    pub fn battle_seed(&self) -> u64 {
        self.fight.battle_id.unwrap_or_default().max(0) as u64
    }

    pub fn card_hand(&self) -> &[CardInfo] {
        self.managers.card.hand()
    }

    pub fn card_deck(&self, team_type: i32) -> Option<&[CardInfo]> {
        match team_type {
            0 => Some(self.managers.card.draw_pile()),
            1 => Some(self.managers.card.ai_queue()),
            _ => None,
        }
    }

    /// Seeds externally observed random draws without bypassing legal draw candidates.
    pub fn seed_card_draws(&mut self, cards: Vec<CardInfo>) {
        self.determinism.enqueue_card_draws(cards);
    }

    pub fn seed_next_ai_cards(&mut self, cards: Vec<CardInfo>) {
        self.determinism.enqueue_next_ai_card_snapshot(cards);
    }

    pub fn seed_hidden_crits(
        &mut self,
        skill_id: i32,
        source_uid: i64,
        choices: impl IntoIterator<Item = bool>,
    ) {
        self.determinism
            .enqueue_hidden_crits(skill_id, source_uid, choices);
    }

    pub fn conduit_operations(&self) -> Vec<sonettobuf::FightDeviceOper> {
        self.managers
            .conduit
            .selections()
            .into_iter()
            .map(|(uid, index)| sonettobuf::FightDeviceOper {
                uid: Some(uid),
                index: Some(index),
            })
            .collect()
    }

    pub fn entity_info(&self, uid: i64) -> Option<sonettobuf::FightEntityInfo> {
        self.managers.entity_snapshot(uid)
    }

    pub fn attack_statistics(&self) -> Vec<FightStatistics> {
        self.fight
            .attacker
            .iter()
            .flat_map(|team| team.entitys.iter().chain(&team.sub_entitys))
            .filter_map(|entity| entity.uid)
            .map(|hero_uid| FightStatistics {
                hero_uid: Some(hero_uid),
                harm: Some(self.managers.hp.total_damage_dealt(hero_uid)),
                hurt: Some(self.managers.hp.total_damage_taken(hero_uid)),
                heal: Some(self.managers.hp.total_healing_done(hero_uid)),
                cards: self
                    .managers
                    .card
                    .played_skill_counts(hero_uid)
                    .into_iter()
                    .map(|(skill_id, use_count)| UseCardStatistics {
                        skill_id: Some(skill_id),
                        use_count: Some(use_count),
                    })
                    .collect(),
                get_buffs: self
                    .managers
                    .buff
                    .added_history_for_owner(hero_uid)
                    .to_vec(),
            })
            .collect()
    }

    pub fn extend_battle_rule_skills(
        &mut self,
        skills: impl IntoIterator<Item = crate::engine::fight::rules::OwnedBattleSkill>,
    ) {
        let skills = skills.into_iter().collect::<Vec<_>>();
        self.catalog.extend_roots_and_warn(
            config::configs::get(),
            skills.iter().map(|skill| skill.skill_id),
            std::iter::empty(),
        );
        self.managers.battle_rule.extend_owned_skills(skills);
    }

    /// Builds a runtime by seeding every manager and exact skill catalog from a fight.
    pub fn new(fight: Fight) -> Self {
        Self::new_with_ex_attributes(fight, std::iter::empty())
    }

    pub fn new_with_ex_attributes(
        fight: Fight,
        ex_attributes: impl IntoIterator<Item = (i64, HeroExAttribute)>,
    ) -> Self {
        Self::new_with_attributes(fight, ex_attributes, std::iter::empty())
    }

    /// Builds a runtime and applies persisted extended and special attributes.
    pub fn new_with_attributes(
        fight: Fight,
        ex_attributes: impl IntoIterator<Item = (i64, HeroExAttribute)>,
        sp_attributes: impl IntoIterator<Item = (i64, HeroSpAttribute)>,
    ) -> Self {
        let mut managers = BattleManagers::seeded(&fight);
        for (uid, attributes) in ex_attributes {
            managers.attribute.override_ex(uid, &attributes);
        }
        for (uid, attributes) in sp_attributes {
            managers.attribute.override_sp(uid, &attributes);
        }
        managers.attribute.sync_emitter_average(&fight);
        let round_state = RoundState::start(&fight);
        let determinism =
            RoundDeterminism::with_seed(fight.battle_id.unwrap_or_default().max(0) as u64);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

        Self {
            fight,
            managers,
            catalog,
            round: None,
            round_state,
            determinism,
            pending_redeal: None,
            cloth_skill_uses: HashMap::new(),
        }
    }
}

fn project_result(
    result: drain::DrainResult,
    fight_version: i32,
) -> Result<Vec<sonettobuf::FightStep>, String> {
    crate::engine::packet::timeline::project_for_version(&result.frames, fight_version)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod test;
