use std::collections::{HashMap, HashSet, VecDeque};

use sonettobuf::CardInfo;

use crate::engine::manager::card::CardPlayChoice;

type StartDeckSeed = (Vec<CardInfo>, Vec<CardInfo>, Vec<CardInfo>, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillTargetChoice {
    pub skill_id: i32,
    pub source_uid: i64,
    pub target_code: i32,
    pub targets: Vec<i64>,
    pub additional_targets: Vec<i64>,
    pub crit_targets: Vec<i64>,
    pub additional_crit_targets: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiSkillChoice {
    pub source_uid: i64,
    pub skill_id: i32,
    pub target_uid: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionRandomChoice {
    pub skill_id: i32,
    pub opcode: i32,
    pub roll: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandRankChoice {
    pub opcode: i32,
    pub owner_uid: i64,
    pub hand_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandCardChoice {
    pub skill_id: i32,
    pub opcode: i32,
    pub hand_index: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RoundDeterminism {
    pub skill_targets: Vec<SkillTargetChoice>,
    start_decks: VecDeque<StartDeckSeed>,
    card_draws: VecDeque<CardInfo>,
    card_energy_snapshots: VecDeque<Vec<CardInfo>>,
    crystal_cards: VecDeque<CardInfo>,
    card_plays: VecDeque<CardPlayChoice>,
    condition_random_choices: Vec<ConditionRandomChoice>,
    hidden_crit_choices: HashMap<(i32, i64), VecDeque<bool>>,
    additional_crit_choices: HashMap<(i32, i64, i64), VecDeque<bool>>,
    scripted_condition_random: HashSet<(i32, i32)>,
    next_ai_card_snapshots: VecDeque<Vec<CardInfo>>,
    ai_skill_snapshots: VecDeque<Vec<AiSkillChoice>>,
    random_skill_choices: VecDeque<i32>,
    scripted_random_skills: HashSet<i32>,
    random_buff_choices: VecDeque<i32>,
    hand_rank_choices: VecDeque<HandRankChoice>,
    hand_card_choices: VecDeque<HandCardChoice>,
    permille_rolls: VecDeque<i32>,
    crit_seed: u64,
    crit_roll_index: u64,
    card_seed: u64,
    card_roll_index: u64,
    lua_random_state: Option<[u64; 4]>,
}

impl RoundDeterminism {
    pub fn with_seed(seed: u64) -> Self {
        Self {
            crit_seed: seed,
            card_seed: seed,
            ..Default::default()
        }
    }

    pub fn enqueue_start_decks(&mut self, ai_deck: Vec<CardInfo>, player_deck: Vec<CardInfo>) {
        self.start_decks
            .push_back((ai_deck, player_deck, Vec::new(), 0));
    }

    pub fn enqueue_opening_seed(
        &mut self,
        ai_deck: Vec<CardInfo>,
        player_deck: Vec<CardInfo>,
        card_draws: Vec<CardInfo>,
        reserved_ultimate_slots: usize,
    ) {
        self.start_decks
            .push_back((ai_deck, player_deck, card_draws, reserved_ultimate_slots));
    }

    pub fn take_start_decks(&mut self) -> Option<StartDeckSeed> {
        self.start_decks.pop_front()
    }

    pub fn enqueue_card_draws(&mut self, cards: Vec<CardInfo>) {
        self.card_draws.extend(cards);
    }

    pub fn has_queued_card_draw(&self) -> bool {
        !self.card_draws.is_empty()
    }

    pub fn clear_card_draws(&mut self) {
        self.card_draws.clear();
    }

    pub fn draw_cards(&mut self, candidates: &[CardInfo], count: usize) -> Vec<CardInfo> {
        (0..count)
            .filter_map(|_| {
                if let Some(queued) = self.card_draws.pop_front()
                    && let Some(candidate) = candidates
                        .iter()
                        .find(|candidate| {
                            candidate.uid == queued.uid && candidate.skill_id == queued.skill_id
                        })
                        .cloned()
                {
                    return Some(candidate);
                }
                let index =
                    random_index(self.card_seed, &mut self.card_roll_index, candidates.len())?;
                Some(candidates[index].clone())
            })
            .collect()
    }

    pub fn enqueue_card_energy_snapshot(&mut self, cards: Vec<CardInfo>) {
        if !cards.is_empty() {
            self.card_energy_snapshots.push_back(cards);
        }
    }

    pub fn take_card_energy_snapshot(
        &mut self,
        cards: &[CardInfo],
        expected_energy: i32,
    ) -> Option<Vec<CardInfo>> {
        let captured = self.card_energy_snapshots.pop_front()?;
        if captured.len() != cards.len()
            || captured.iter().zip(cards).any(|(captured, current)| {
                captured.uid != current.uid || captured.skill_id != current.skill_id
            })
            || captured
                .iter()
                .any(|card| !(0..=5).contains(&card.energy.unwrap_or_default()))
            || captured.iter().zip(cards).any(|(captured, current)| {
                captured.energy.unwrap_or_default() < current.energy.unwrap_or_default()
            })
            || captured
                .iter()
                .zip(cards)
                .map(|(captured, current)| {
                    captured.energy.unwrap_or_default() - current.energy.unwrap_or_default()
                })
                .sum::<i32>()
                != expected_energy
        {
            return None;
        }
        Some(
            cards
                .iter()
                .zip(captured)
                .map(|(current, captured)| CardInfo {
                    energy: captured.energy,
                    ..current.clone()
                })
                .collect(),
        )
    }

    pub fn enqueue_crystal_cards(&mut self, cards: impl IntoIterator<Item = CardInfo>) {
        self.crystal_cards.extend(cards);
    }

    pub fn take_crystal_card(&mut self, skill_ids: &[i32]) -> Option<CardInfo> {
        let index = self.crystal_cards.iter().position(|card| {
            card.skill_id
                .is_some_and(|skill_id| skill_ids.contains(&skill_id))
        })?;
        self.crystal_cards.remove(index)
    }

    pub fn enqueue_card_plays(&mut self, choices: Vec<CardPlayChoice>) {
        self.card_plays.extend(choices);
    }

    pub(crate) fn card_play_skill_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.card_plays
            .iter()
            .filter_map(|choice| choice.played.skill_id)
            .filter(|skill_id| *skill_id > 0)
    }

    pub fn take_card_play(&mut self) -> Option<CardPlayChoice> {
        self.card_plays.pop_front()
    }

    pub fn enqueue_condition_random_choices(&mut self, choices: Vec<ConditionRandomChoice>) {
        self.scripted_condition_random.extend(
            choices
                .iter()
                .map(|choice| (choice.skill_id, choice.opcode)),
        );
        self.condition_random_choices.extend(choices);
    }

    pub fn script_condition_randoms(&mut self, keys: impl IntoIterator<Item = (i32, i32)>) {
        self.scripted_condition_random.extend(keys);
    }

    pub fn enqueue_hidden_crits(
        &mut self,
        skill_id: i32,
        source_uid: i64,
        choices: impl IntoIterator<Item = bool>,
    ) {
        self.hidden_crit_choices
            .entry((skill_id, source_uid))
            .or_default()
            .extend(choices);
    }

    pub fn enqueue_skill_target_choices(
        &mut self,
        choices: impl IntoIterator<Item = SkillTargetChoice>,
    ) {
        for choice in choices {
            let crits = choice
                .targets
                .iter()
                .map(|target_uid| choice.crit_targets.contains(target_uid))
                .collect::<Vec<_>>();
            self.enqueue_hidden_crits(choice.skill_id, choice.source_uid, crits);
            for target_uid in &choice.additional_targets {
                self.additional_crit_choices
                    .entry((choice.skill_id, choice.source_uid, *target_uid))
                    .or_default()
                    .push_back(choice.additional_crit_targets.contains(target_uid));
            }
            self.skill_targets.push(choice);
        }
    }

    pub fn roll_hidden_crit(
        &mut self,
        skill_id: i32,
        source_uid: i64,
        target_uid: i64,
        chance: i32,
    ) -> bool {
        self.hidden_crit_choices
            .get_mut(&(skill_id, source_uid))
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| self.roll_crit(skill_id, source_uid, target_uid, chance))
    }

    pub fn roll_additional_crit(
        &mut self,
        skill_id: i32,
        action_source_uid: i64,
        credited_source_uid: i64,
        target_uid: i64,
        chance: i32,
    ) -> bool {
        self.additional_crit_choices
            .get_mut(&(skill_id, action_source_uid, target_uid))
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| self.roll_crit(skill_id, credited_source_uid, target_uid, chance))
    }

    pub fn condition_random_roll(&mut self, skill_id: i32, opcode: i32) -> i32 {
        if let Some(index) = self
            .condition_random_choices
            .iter()
            .position(|choice| choice.skill_id == skill_id && choice.opcode == opcode)
        {
            return self
                .condition_random_choices
                .remove(index)
                .roll
                .clamp(0, 999);
        }
        // captures only expose successful activations; exhausted captured streams fail
        // closed. Record ordered failures too if a future fixture needs success-after-failure parity.
        if self.scripted_condition_random.contains(&(skill_id, opcode)) {
            return 999;
        }
        self.lua_random_index(1000).unwrap_or_default() as i32
    }

    pub fn roll_permille(&mut self, chance: i32) -> bool {
        let roll = self
            .permille_rolls
            .pop_front()
            .map(|roll| roll.clamp(0, 999))
            .unwrap_or_else(|| self.lua_random_index(1000).unwrap_or_default() as i32);
        chance >= 1000 || (chance > 0 && roll < chance)
    }

    pub fn enqueue_permille_rolls(&mut self, rolls: impl IntoIterator<Item = i32>) {
        self.permille_rolls.extend(rolls);
    }

    pub fn enqueue_next_ai_card_snapshot(&mut self, cards: Vec<CardInfo>) {
        if !cards.is_empty() {
            self.next_ai_card_snapshots.push_back(cards);
        }
    }

    pub fn take_next_ai_card_snapshot(&mut self) -> Option<Vec<CardInfo>> {
        self.next_ai_card_snapshots.pop_front()
    }

    pub fn enqueue_ai_skills(&mut self, skills: Vec<AiSkillChoice>) {
        if !skills.is_empty() {
            self.ai_skill_snapshots.push_back(skills);
        }
    }

    pub fn take_ai_skills(&mut self) -> Option<Vec<AiSkillChoice>> {
        self.ai_skill_snapshots.pop_front()
    }

    pub fn enqueue_random_skills(&mut self, skills: impl IntoIterator<Item = i32>) {
        let skills = skills.into_iter().collect::<Vec<_>>();
        self.scripted_random_skills.extend(skills.iter().copied());
        self.random_skill_choices.extend(skills);
    }

    pub fn take_random_skill(&mut self, candidates: &[i32]) -> Option<i32> {
        candidates
            .contains(self.random_skill_choices.front()?)
            .then(|| self.random_skill_choices.pop_front())
            .flatten()
    }

    pub fn has_scripted_random_skill(&self, candidates: &[i32]) -> bool {
        candidates
            .iter()
            .any(|skill_id| self.scripted_random_skills.contains(skill_id))
    }

    pub fn enqueue_random_buffs(&mut self, buffs: impl IntoIterator<Item = i32>) {
        self.random_buff_choices.extend(buffs);
    }

    pub fn take_random_buff(&mut self, candidates: &[i32]) -> Option<i32> {
        candidates
            .contains(self.random_buff_choices.front()?)
            .then(|| self.random_buff_choices.pop_front())
            .flatten()
    }

    pub fn enqueue_hand_rank_choices(&mut self, choices: impl IntoIterator<Item = HandRankChoice>) {
        self.hand_rank_choices.extend(choices);
    }

    pub fn take_hand_rank_choice(
        &mut self,
        opcode: i32,
        owner_uid: i64,
        candidates: &[usize],
    ) -> Option<usize> {
        let index = self.hand_rank_choices.iter().position(|choice| {
            choice.opcode == opcode
                && choice.owner_uid == owner_uid
                && candidates.contains(&choice.hand_index)
        })?;
        self.hand_rank_choices
            .remove(index)
            .map(|choice| choice.hand_index)
    }

    pub fn enqueue_hand_card_choices(&mut self, choices: impl IntoIterator<Item = HandCardChoice>) {
        self.hand_card_choices.extend(choices);
    }

    pub fn take_hand_card_choice(
        &mut self,
        skill_id: i32,
        opcode: i32,
        candidates: &[usize],
    ) -> Option<usize> {
        let index = self.hand_card_choices.iter().position(|choice| {
            choice.skill_id == skill_id
                && choice.opcode == opcode
                && candidates.contains(&choice.hand_index)
        })?;
        self.hand_card_choices
            .remove(index)
            .map(|choice| choice.hand_index)
    }

    pub fn take_skill_targets(
        &mut self,
        skill_id: i32,
        source_uid: i64,
        target_code: i32,
    ) -> Option<Vec<i64>> {
        self.skill_targets
            .iter()
            .find(|choice| {
                choice.skill_id == skill_id
                    && choice.source_uid == source_uid
                    && (choice.target_code == target_code
                        || (choice.target_code == -1 && target_code != 0))
            })
            .map(|choice| choice.targets.clone())
    }

    pub fn take_skill_target_choice(
        &mut self,
        skill_id: i32,
        source_uid: i64,
        target_code: i32,
    ) -> Option<SkillTargetChoice> {
        let index = self.skill_targets.iter().position(|choice| {
            choice.skill_id == skill_id
                && choice.source_uid == source_uid
                && (choice.target_code == target_code
                    || (choice.target_code == -1 && target_code != 0))
        })?;
        Some(self.skill_targets.remove(index))
    }

    pub fn roll_crit(
        &mut self,
        skill_id: i32,
        source_uid: i64,
        target_uid: i64,
        chance: i32,
    ) -> bool {
        let mut value = (skill_id as u64)
            ^ (source_uid as u64).rotate_left(17)
            ^ (target_uid as u64).rotate_left(37)
            ^ self.crit_seed
            ^ self.crit_roll_index;
        self.crit_roll_index = self.crit_roll_index.wrapping_add(1);
        value ^= value >> 30;
        value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 27;
        value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
        (value % 1000) < chance.clamp(0, 1000) as u64
    }

    pub fn lua_random_index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let state = self.lua_random_state.get_or_insert_with(|| {
            let mut state = [0, 0xff, 0, 0];
            for _ in 0..16 {
                lua_next_random(&mut state);
            }
            state
        });
        Some((lua_next_random(state) % len as u64) as usize)
    }
}

fn random_index(seed: u64, roll_index: &mut u64, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let mut value = seed
        .wrapping_add(*roll_index)
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    *roll_index = roll_index.wrapping_add(1);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    Some(((value ^ (value >> 31)) % len as u64) as usize)
}

fn lua_next_random(state: &mut [u64; 4]) -> u64 {
    let result = state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
    let t = state[1] << 17;
    state[2] ^= state[0];
    state[3] ^= state[1];
    state[1] ^= state[2];
    state[0] ^= state[3];
    state[2] ^= t;
    state[3] = state[3].rotate_left(45);
    result
}

#[cfg(test)]
mod tests;
