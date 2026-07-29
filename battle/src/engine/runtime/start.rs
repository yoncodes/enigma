use sonettobuf::{CardInfo, CardInfoPush, Fight, FightRound, FightStep};

use crate::engine::{
    manager::{
        BattleManagers,
        card::{CardCommand, CardSetAiQueue, CardSetTeamCards, CardSetup},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
    },
};

use super::{BattleRuntime, determinism::RoundDeterminism, schedule};

fn run_start_schedule(
    fight: &Fight,
    managers: &mut BattleManagers,
    catalog: &SkillEffectCatalog,
    cards: CardSetup,
    determinism: &mut RoundDeterminism,
    hand_size: usize,
) -> Result<(Vec<FightStep>, Vec<CardInfo>), String> {
    let pool = crate::engine::skill::target::TargetPool::from_fight(fight);
    let context = crate::engine::skill::target::TargetContext {
        battle_id: fight.battle_id.unwrap_or_default(),
        current_round: 1,
        ..Default::default()
    };
    managers.gauge.begin_opening_setup();
    let result = schedule::run_start(
        managers,
        &pool,
        catalog,
        determinism,
        context,
        cards,
        hand_size,
    );
    managers.gauge.finish_opening_setup();
    let (result, visible_cards) = result.map_err(|error| format!("{error:?}"))?;
    let steps = crate::engine::packet::timeline::project_for_version(
        &result.frames,
        fight.version.unwrap_or_default(),
    )
    .map_err(|error| format!("{error:?}"))?;
    Ok((steps, visible_cards))
}

impl BattleRuntime {
    pub fn start_round(&mut self) -> Result<FightRound, String> {
        let round = self.build_start_round_from_schedule()?;
        self.round = Some(round.clone());
        Ok(round)
    }

    pub fn start_round_with_determinism(
        &mut self,
        determinism: RoundDeterminism,
    ) -> Result<FightRound, String> {
        self.determinism = determinism;
        self.start_round()
    }

    pub(crate) fn start_state(&self) -> (&Fight, Option<&FightRound>) {
        (&self.fight, self.round.as_ref())
    }

    pub fn build_start_steps(&mut self, cards: CardSetup) -> Result<Vec<FightStep>, String> {
        self.build_start_schedule(cards, None)
            .map(|(steps, _)| steps)
    }

    fn build_start_schedule(
        &mut self,
        cards: CardSetup,
        determinism: Option<&mut RoundDeterminism>,
    ) -> Result<(Vec<FightStep>, Vec<CardInfo>), String> {
        let hand_size = crate::engine::manager::card::start::hand_size(&self.fight);
        let determinism = determinism.unwrap_or(&mut self.determinism);
        run_start_schedule(
            &self.fight,
            &mut self.managers,
            &self.catalog,
            cards,
            determinism,
            hand_size,
        )
    }

    pub fn build_start_steps_with_determinism(
        &mut self,
        cards: CardSetup,
        determinism: &mut RoundDeterminism,
    ) -> Result<Vec<FightStep>, String> {
        self.build_start_schedule(cards, Some(determinism))
            .map(|(steps, _)| steps)
    }

    pub(super) fn build_start_round_from_schedule(&mut self) -> Result<FightRound, String> {
        let battle_id = self.fight.battle_id.unwrap_or_default();
        let (ai_deck, player_deck) = crate::engine::manager::card::start_decks_from_fight(
            &self.fight,
            &self.managers.ex_point,
            battle_id,
            self.determinism.take_start_decks(),
        );
        self.catalog.extend_roots_and_warn(
            config::configs::get(),
            ai_deck.iter().filter_map(|card| card.skill_id),
            std::iter::empty(),
        );
        let opening_hand_size = player_deck
            .iter()
            .filter(|card| !card.temp_card.unwrap_or_default())
            .count();
        let (opening_deal, preserve_refill_floor) = if let Some(configured) =
            crate::engine::manager::card::start::configured_opening_deal(&self.fight)?
        {
            (configured, true)
        } else {
            let drawn = self.determinism.draw_cards(
                &available_player_cards(&self.fight, &self.managers),
                opening_hand_size,
            );
            if drawn.len() == opening_hand_size {
                (drawn, false)
            } else {
                (player_deck.clone(), false)
            }
        };
        self.determinism.enqueue_card_draws(
            crate::engine::manager::card::start::configured_refill_draws(&self.fight)?,
        );
        let opening_pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
        let opening_team_cards = crate::engine::mechanic::card::CardMechanic.special_team_cards(
            &opening_pool,
            &self.managers,
            &opening_deal,
        );
        let draw_pile = crate::engine::manager::card::start::draw_bag(&self.fight);
        let deck_num = crate::engine::manager::card::start::deck_size(&self.fight);
        self.managers
            .execute_card(CardCommand::SetAiQueue(CardSetAiQueue {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "OpeningAiQueue"),
                },
                cards: ai_deck,
            }))
            .map_err(|error| format!("{error:?}"))?;
        let (steps, mut visible_cards) = self.build_start_schedule(
            CardSetup {
                hand: opening_deal,
                draw_pile,
                deck_num,
            },
            None,
        )?;
        if preserve_refill_floor {
            self.managers
                .execute_card(CardCommand::PreserveRefillFloor)
                .map_err(|error| format!("{error:?}"))?;
        }
        self.managers
            .execute_card(CardCommand::SetTeamCards(CardSetTeamCards {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "OpeningTeamCards"),
                },
                cards: opening_team_cards.clone(),
            }))
            .map_err(|error| format!("{error:?}"))?;
        visible_cards.extend(opening_team_cards);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&self.fight);
        self.round_state.act_point = crate::engine::round::state::next_action_points(
            &self.fight,
            &pool,
            &self.managers,
            &self.catalog,
            &mut self.determinism,
            crate::engine::skill::target::TargetContext {
                battle_id,
                current_round: self.round_state.cur_round,
                ..Default::default()
            },
        );
        self.round_state.hero_sp_attributes = self.managers.hero_sp_attributes(&self.fight);
        self.round_state.last_change_hero_uid = self.fight.last_change_hero_uid.or(Some(0));
        let attacker = self.fight.attacker.as_ref();
        Ok(FightRound {
            act_point: Some(self.round_state.act_point),
            is_finish: Some(self.round_state.is_finish),
            move_num: Some(self.round_state.move_num),
            ai_use_cards: self.managers.card.ai_queue().to_vec(),
            power: Some(self.round_state.power),
            skill_infos: attacker
                .map(|team| team.skill_infos.clone())
                .unwrap_or_default(),
            before_cards1: Vec::new(),
            team_a_cards1: visible_cards,
            before_cards2: Vec::new(),
            team_a_cards2: Vec::new(),
            next_round_begin_step: Vec::new(),
            use_card_list: Vec::new(),
            cur_round: Some(self.round_state.cur_round),
            hero_sp_attributes: self.round_state.hero_sp_attributes.clone(),
            last_change_hero_uid: self.round_state.last_change_hero_uid,
            ex_point_info: self.managers.ex_point_info(&self.fight),
            fight_step: steps,
            total_step: None,
            fight_step_bytes: None,
        })
    }

    pub fn card_info_push(&self) -> CardInfoPush {
        let cards = self
            .managers
            .card
            .hand()
            .iter()
            .chain(self.managers.card.team_cards())
            .cloned()
            .collect::<Vec<_>>();
        let deal_cards = self
            .round
            .as_ref()
            .map(|round| round.team_a_cards1.as_slice())
            .unwrap_or(&cards)
            .iter()
            .map(|card| CardInfo {
                energy: Some(0),
                ..card.clone()
            })
            .collect();
        CardInfoPush {
            card_group: cards,
            act_point: Some(self.round_state.act_point),
            move_num: Some(0),
            before_cards: Vec::new(),
            deal_card_group: deal_cards,
            extra_move_act: Some(0),
            is_gm: Some(false),
        }
    }
}

pub(super) fn available_player_cards(fight: &Fight, managers: &BattleManagers) -> Vec<CardInfo> {
    crate::engine::manager::card::pool::player_candidate_pool_with(fight, |entity| {
        let owner_uid = entity.uid.unwrap_or_default();
        managers.ex_point.is_full(owner_uid)
            && !managers.buff.has_buff_act_kind(
                owner_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::CantGetExskill,
            )
    })
}
