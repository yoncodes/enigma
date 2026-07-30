use crate::{
    GameDB, character_cosume::CharacterCosume, character_data::CharacterData,
    character_destiny::CharacterDestiny,
    character_destiny_facets_consume::CharacterDestinyFacetsConsume,
    character_destiny_slots::CharacterDestinySlots, character_level::CharacterLevel,
    character_rank::CharacterRank, character_talent::CharacterTalent,
    character_voice::CharacterVoice, hero3124_skill_talent::Hero3124SkillTalent, skin::Skin,
    talent_scheme::TalentScheme, talent_style_cost::TalentStyleCost,
};

impl GameDB {
    pub fn max_faith(&self) -> i32 {
        self.friendless.iter().map(|row| row.friendliness).sum()
    }

    pub fn faith_percent(&self, faith: i32) -> i32 {
        let mut accumulated = 0;
        let mut percent = 0;

        for level in self.friendless.iter() {
            accumulated += level.friendliness;
            if faith < accumulated {
                return percent;
            }
            percent = level.percentage;
            if faith == accumulated {
                return percent;
            }
        }

        100
    }

    pub fn talent_scheme(&self, talent_id: i32, talent_mould: i32) -> Option<&TalentScheme> {
        self.talent_scheme
            .iter()
            .find(|row| row.talent_id == talent_id && row.talent_mould == talent_mould)
    }

    pub fn starting_character_level(&self, hero_id: i32) -> Option<&CharacterLevel> {
        self.character_level
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .min_by_key(|row| row.level)
    }

    pub fn character_level(&self, hero_id: i32, level: i32) -> Option<&CharacterLevel> {
        self.character_level
            .iter()
            .find(|row| row.hero_id == hero_id && row.level == level)
    }

    pub fn character_rank_level_limit(&self, hero_id: i32, rank: i32) -> Option<i32> {
        self.character_rank(hero_id, rank)?
            .effect
            .split('|')
            .filter_map(|entry| entry.split_once('#'))
            .find_map(|(kind, value)| (kind == "1").then(|| value.parse().ok()).flatten())
    }

    pub fn character_rank(&self, hero_id: i32, rank: i32) -> Option<&CharacterRank> {
        self.character_rank
            .iter()
            .find(|row| row.hero_id == hero_id && row.rank == rank)
    }

    pub fn character_level_cost(&self, rare: i32, level: i32) -> Option<&CharacterCosume> {
        self.character_cosume
            .iter()
            .find(|row| row.rare == rare && row.level == level)
    }

    pub fn max_character_level(&self) -> i32 {
        self.character_level
            .iter()
            .map(|row| row.level)
            .max()
            .unwrap_or_default()
    }

    pub fn starting_character_rank(&self, hero_id: i32) -> Option<&CharacterRank> {
        self.character_rank
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .min_by_key(|row| row.rank)
    }

    pub fn character_talent(&self, hero_id: i32, talent_id: i32) -> Option<&CharacterTalent> {
        self.character_talent
            .iter()
            .find(|row| row.hero_id == hero_id && row.talent_id == talent_id)
    }

    pub fn character_voices(&self, hero_id: i32) -> impl Iterator<Item = &CharacterVoice> {
        self.character_voice
            .iter()
            .filter(move |row| row.hero_id == hero_id)
    }

    pub fn character_unlock_item(&self, hero_id: i32, item_id: i32) -> Option<&CharacterData> {
        self.character_data.iter().find(|row| {
            row.hero_id == hero_id
                && row.id == item_id
                && row.r#type == 2
                && !row.unlock_rewards.is_empty()
        })
    }

    pub fn character_destiny(&self, hero_id: i32) -> Option<&CharacterDestiny> {
        self.character_destiny
            .iter()
            .find(|row| row.hero_id == hero_id)
    }

    pub fn character_destiny_slot(
        &self,
        slots_id: i32,
        stage: i32,
        node: i32,
    ) -> Option<&CharacterDestinySlots> {
        self.character_destiny_slots
            .iter()
            .find(|row| row.slots_id == slots_id && row.stage == stage && row.node == node)
    }

    pub fn character_destiny_stone_cost(
        &self,
        stone_id: i32,
    ) -> Option<&CharacterDestinyFacetsConsume> {
        self.character_destiny_facets_consume
            .iter()
            .find(|row| row.facets_id == stone_id)
    }

    pub fn character_unique_skill_kind(&self, hero_id: i32) -> Option<i32> {
        self.character
            .get(hero_id)?
            .unique_skill_point
            .split_once('#')?
            .0
            .parse()
            .ok()
    }

    pub fn has_character_weapon(&self, main_id: i32, sub_id: i32, skill_level: i32) -> bool {
        self.fight_eziozhuangbei.iter().any(|row| {
            row.first_id == main_id && row.second_id == sub_id && row.skill_level == skill_level
        })
    }

    pub fn hero_skill_talent(&self, sub_id: i32, level: i32) -> Option<&Hero3124SkillTalent> {
        self.hero3124_skill_talent
            .iter()
            .find(|row| row.sub == sub_id && row.level == level)
    }

    pub fn hero_skill_talent_level(&self, sub_id: i32, talent_id: i32) -> Option<i32> {
        self.hero3124_skill_talent
            .iter()
            .find(|row| row.sub == sub_id && row.talent_id == talent_id)
            .map(|row| row.level)
    }

    pub fn default_character_skin(&self, hero_id: i32) -> Option<&Skin> {
        self.skin
            .iter()
            .filter(|row| row.character_id == hero_id)
            .min_by_key(|row| row.id)
    }

    pub fn talent_style_cost(&self, hero_id: i32, style_id: i32) -> Option<&TalentStyleCost> {
        self.talent_style_cost
            .iter()
            .find(|row| row.hero_id == hero_id && row.style_id == style_id)
    }
}
