use config::configs;
use database::db::game::equipment::Equipment;
use database::models::game::heros::HeroData;
use std::collections::HashMap;

use super::destiny::Destiny;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PassiveSourceKind {
    #[default]
    Unknown,
    Insight,
    Psychube,
    Destiny,
    BattleRule,
    Extra,
    Rank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PassiveSource {
    pub kind: PassiveSourceKind,
    /// Insight/skill level for `Insight`, destiny rank for `Destiny`.
    pub rank: i32,
    /// Destiny stone id for `Destiny`, equip id for `Psychube`.
    pub source_id: i32,
}

impl PassiveSource {
    pub fn new(kind: PassiveSourceKind) -> Self {
        Self {
            kind,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassiveSkill {
    pub skill_id: i32,
    pub source: PassiveSource,
}

pub struct Passive;

impl Passive {
    pub fn get(
        hero_data: &HeroData,
        equips: &[Equipment],
        destiny: Option<&HashMap<i32, i32>>,
    ) -> Vec<PassiveSkill> {
        let r = &hero_data.record;
        let mut passives = Self::base(r.hero_id);
        Self::apply_upgrades(
            &mut passives,
            r.hero_id,
            r.ex_skill_level,
            destiny,
            r.destiny_rank,
            r.destiny_stone,
        );
        for equip in equips {
            passives.extend(Self::psychube(equip.equip_id, Some(equip.refine_lv)));
        }
        passives
    }

    pub fn for_config(
        hero_id: i32,
        psychube: Option<(i32, i32)>,
        destiny: Option<(i32, i32)>,
    ) -> Vec<PassiveSkill> {
        let ex_level = configs::get()
            .skill_ex_level
            .iter()
            .filter(|row| row.hero_id == hero_id)
            .map(|row| row.skill_level)
            .max()
            .unwrap_or_default();
        Self::for_loadout(hero_id, ex_level, psychube, destiny)
    }

    pub fn for_loadout(
        hero_id: i32,
        ex_level: i32,
        psychube: Option<(i32, i32)>,
        destiny: Option<(i32, i32)>,
    ) -> Vec<PassiveSkill> {
        let (destiny_stone, destiny_rank) = destiny.unwrap_or_default();
        let destiny = Destiny::get(destiny_stone, destiny_rank);
        let mut passives = Self::base(hero_id);
        Self::apply_upgrades(
            &mut passives,
            hero_id,
            ex_level,
            destiny.as_ref(),
            destiny_rank,
            destiny_stone,
        );
        if let Some((psychube_id, psychube_level)) = psychube {
            passives.extend(Self::psychube(psychube_id, Some(psychube_level)));
        }
        passives
    }

    pub fn for_ranked_loadout(
        hero_id: i32,
        rank: i32,
        ex_level: i32,
        psychube: Option<(i32, i32)>,
        destiny: Option<(i32, i32)>,
    ) -> Vec<PassiveSkill> {
        let game = configs::get();
        let insight_level = game
            .character_rank
            .iter()
            .find(|row| row.hero_id == hero_id && row.rank == rank)
            .and_then(|row| {
                row.effect
                    .split('|')
                    .filter_map(|effect| effect.split_once('#'))
                    .find_map(|(kind, value)| (kind == "2").then(|| value.parse().ok()).flatten())
            })
            .unwrap_or_default();
        let mut passives = Self::base(hero_id)
            .into_iter()
            .filter(|passive| {
                passive.source.kind != PassiveSourceKind::Insight
                    || passive.source.rank <= insight_level
            })
            .collect::<Vec<_>>();
        passives.extend(
            game.character_rank
                .iter()
                .filter(|row| row.hero_id == hero_id && row.rank < rank)
                .flat_map(|row| row.effect.split('|'))
                .filter_map(|effect| effect.split_once('#'))
                .filter_map(|(kind, skill_id)| {
                    (kind == "5").then(|| skill_id.parse().ok()).flatten()
                })
                .map(|skill_id| PassiveSkill {
                    skill_id,
                    source: PassiveSource::new(PassiveSourceKind::Rank),
                }),
        );

        let (destiny_stone, destiny_rank) = destiny.unwrap_or_default();
        let destiny = Destiny::get(destiny_stone, destiny_rank);
        Self::apply_upgrades(
            &mut passives,
            hero_id,
            ex_level,
            destiny.as_ref(),
            destiny_rank,
            destiny_stone,
        );
        if let Some((psychube_id, psychube_level)) = psychube {
            passives.extend(Self::psychube(psychube_id, Some(psychube_level)));
        }
        passives
    }

    fn base(hero_id: i32) -> Vec<PassiveSkill> {
        if let Some(base_ids) = Self::activity_base(hero_id) {
            return base_ids
                .into_iter()
                .map(|skill_id| PassiveSkill {
                    skill_id,
                    source: PassiveSource::new(PassiveSourceKind::Extra),
                })
                .collect();
        }
        let mut rows = configs::get()
            .skill_passive_level
            .iter()
            .filter(|row| row.hero_id == hero_id && row.skill_passive != 0)
            .map(|row| (row.skill_level, row.skill_passive))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(level, _)| if *level == 0 { i32::MAX } else { *level });
        rows.into_iter()
            .map(|(level, skill_id)| PassiveSkill {
                skill_id,
                source: PassiveSource {
                    kind: if level == 0 {
                        PassiveSourceKind::Rank
                    } else {
                        PassiveSourceKind::Insight
                    },
                    rank: level,
                    source_id: 0,
                },
            })
            .collect()
    }

    fn apply_upgrades(
        passives: &mut [PassiveSkill],
        hero_id: i32,
        ex_level: i32,
        destiny: Option<&HashMap<i32, i32>>,
        destiny_rank: i32,
        destiny_stone: i32,
    ) {
        let ex_map = Self::build_ex_map(hero_id, ex_level);
        for passive in passives {
            if let Some(&overridden) = destiny.and_then(|map| map.get(&passive.skill_id)) {
                passive.skill_id = overridden;
                passive.source = PassiveSource {
                    kind: PassiveSourceKind::Destiny,
                    rank: destiny_rank,
                    source_id: destiny_stone,
                };
            } else if let Some(&upgraded) = ex_map.get(&passive.skill_id) {
                passive.skill_id = upgraded;
            }
        }
    }

    fn psychube(equip_id: i32, skill_level: Option<i32>) -> Vec<PassiveSkill> {
        let mut rows = configs::get()
            .equip_skill
            .iter()
            .filter(|row| row.id == equip_id);
        let row = if let Some(skill_level) = skill_level {
            rows.find(|row| row.skill_lv == skill_level)
        } else {
            rows.next()
        };
        let Some(row) = row else { return Vec::new() };
        let source = PassiveSource {
            kind: PassiveSourceKind::Psychube,
            rank: row.skill_lv,
            source_id: equip_id,
        };
        [row.skill, row.skill2]
            .into_iter()
            .filter(|skill_id| *skill_id != 0)
            .map(|skill_id| PassiveSkill { skill_id, source })
            .collect()
    }

    fn activity_base(hero_id: i32) -> Option<Vec<i32>> {
        let game = configs::get();

        if let Some(r) = game.activity174_role.iter().find(|r| r.hero_id == hero_id)
            && !r.passive_skill.is_empty()
        {
            return Some(
                r.passive_skill
                    .split('|')
                    .filter_map(|v| v.parse().ok())
                    .collect(),
            );
        }

        if let Some(r) = game.activity191_role.iter().find(|r| r.role_id == hero_id)
            && !r.passive_skill.is_empty()
        {
            return Some(
                r.passive_skill
                    .split('|')
                    .filter_map(|v| v.parse().ok())
                    .collect(),
            );
        }

        None
    }

    fn build_ex_map(hero_id: i32, ex_level: i32) -> HashMap<i32, i32> {
        let game = configs::get();
        let mut map = HashMap::new();

        for lvl in 1..=ex_level {
            if let Some(ex) = game
                .skill_ex_level
                .iter()
                .find(|s| s.hero_id == hero_id && s.skill_level == lvl)
            {
                for pair in ex.passive_skill.split('|') {
                    if let Some((d, a)) = pair.split_once('#')
                        && let (Ok(d), Ok(a)) = (d.parse::<i32>(), a.parse::<i32>())
                    {
                        map.insert(d, a);
                    }
                }
            }
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::init_config;

    #[test]
    fn selected_build_uses_requested_psychube_and_destiny_levels() {
        init_config();
        let passives = Passive::for_config(3086, Some((1527, 4)), Some((308601, 3)));

        assert!(passives.iter().any(|passive| {
            passive.skill_id == 432714
                && passive.source.kind == PassiveSourceKind::Psychube
                && passive.source.rank == 4
        }));
    }
}
