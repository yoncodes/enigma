use crate::{
    GameDB, character_cosume::CharacterCosume, character_data::CharacterData,
    character_destiny::CharacterDestiny,
    character_destiny_facets_consume::CharacterDestinyFacetsConsume,
    character_destiny_slots::CharacterDestinySlots, character_level::CharacterLevel,
    character_rank::CharacterRank, character_talent::CharacterTalent,
    character_voice::CharacterVoice, fight_eziozhuangbei::FightEziozhuangbei,
    hero3124_skill_talent::Hero3124SkillTalent, skin::Skin, talent_scheme::TalentScheme,
    talent_style_cost::TalentStyleCost,
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

    pub fn character_rank_passive_level(&self, hero_id: i32, rank: i32) -> Option<i32> {
        self.character_rank(hero_id, rank)?
            .effect
            .split('|')
            .filter_map(|entry| entry.split_once('#'))
            .find_map(|(kind, value)| (kind == "2").then(|| value.parse().ok()).flatten())
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

    pub fn has_character_weapon(
        &self,
        hero_id: i32,
        main_id: i32,
        sub_id: i32,
        skill_level: i32,
    ) -> bool {
        self.gear_rows(hero_id).any(|row| {
            row.first_id == main_id && row.second_id == sub_id && row.skill_level == skill_level
        })
    }

    /// Gear rows a hero can equip. The table has no hero column and the
    /// resource kind in `uniqueSkill_point` is not an owner; like the client,
    /// the table belongs to Ezio.
    pub fn gear_rows(&self, hero_id: i32) -> impl Iterator<Item = &FightEziozhuangbei> {
        const GEAR_HERO: i32 = 3123;
        self.fight_eziozhuangbei
            .iter()
            .filter(move |_| hero_id == GEAR_HERO)
    }

    /// Gear row selected by a hero's `extraStr` ("first#second").
    pub fn equipped_gear(
        &self,
        hero_id: i32,
        extra_str: &str,
        skill_level: i32,
    ) -> Option<&FightEziozhuangbei> {
        let mut ids = extra_str.split('#').map(|id| id.trim().parse::<i32>().ok());
        let first = ids.next().flatten().filter(|id| *id > 0)?;
        let second = ids.next().flatten().unwrap_or(0);
        self.gear_rows(hero_id).find(|row| {
            row.first_id == first && row.second_id == second && row.skill_level == skill_level
        })
    }

    /// Talent-tree rows a hero can light. Like the client, the table belongs
    /// to Kassandra.
    pub fn talent_tree_rows(&self, hero_id: i32) -> impl Iterator<Item = &Hero3124SkillTalent> {
        const TALENT_TREE_HERO: i32 = 3124;
        self.hero3124_skill_talent
            .iter()
            .filter(move |_| hero_id == TALENT_TREE_HERO)
    }

    /// Talents lit in a hero's `extraStr` ("sub#id,id|sub#id"), in talent-id
    /// order, which is the order the client applies their skill exchanges.
    pub fn lit_talents(&self, hero_id: i32, extra_str: &str) -> Vec<&Hero3124SkillTalent> {
        let lit = extra_str
            .split('|')
            .filter_map(|group| group.split_once('#'))
            .flat_map(|(_, ids)| ids.split(','))
            .filter_map(|id| id.trim().parse::<i32>().ok())
            .collect::<std::collections::BTreeSet<_>>();
        let mut rows = self
            .talent_tree_rows(hero_id)
            .filter(|row| lit.contains(&row.talent_id))
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.talent_id);
        rows
    }

    pub fn talent_skills_at(row: &Hero3124SkillTalent, skill_level: i32) -> (&str, &str) {
        match skill_level {
            0 => (&row.new_skills0, &row.exchange_skills0),
            1 => (&row.new_skills1, &row.exchange_skills1),
            2 => (&row.new_skills2, &row.exchange_skills2),
            3 => (&row.new_skills3, &row.exchange_skills3),
            4 => (&row.new_skills4, &row.exchange_skills4),
            5 => (&row.new_skills5, &row.exchange_skills5),
            _ => ("", ""),
        }
    }

    pub fn talent_exchanges(
        &self,
        hero_id: i32,
        extra_str: &str,
        skill_level: i32,
    ) -> Vec<(i32, i32)> {
        self.lit_talents(hero_id, extra_str)
            .into_iter()
            .flat_map(|row| Self::talent_skills_at(row, skill_level).1.split('|'))
            .filter_map(|pair| {
                let (from, to) = pair.split_once('#')?;
                Some((from.trim().parse().ok()?, to.trim().parse().ok()?))
            })
            .collect()
    }

    pub fn talent_new_skills(&self, hero_id: i32, extra_str: &str, skill_level: i32) -> Vec<i32> {
        self.lit_talents(hero_id, extra_str)
            .into_iter()
            .flat_map(|row| Self::talent_skills_at(row, skill_level).0.split('#'))
            .filter_map(|id| id.trim().parse().ok())
            .collect()
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
