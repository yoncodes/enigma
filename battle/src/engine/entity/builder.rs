use anyhow::{Context, Result};
use sonettobuf::{EnhanceInfoBox, EquipRecord, FightEntityInfo, HeroAttribute, PowerInfo};

use crate::engine::manager::ex_point::ExPointKind;

use super::{
    attr::Attr,
    destiny::Destiny,
    input::{EquipmentBuildInput, HeroBuildInput},
    passive::Passive,
    skill::Skill,
    stats::{BattleBalance, StatInputs, Stats, rank_from_level},
};

pub struct EntityBuilder {
    hero: HeroBuildInput,
    equips: Vec<EquipmentBuildInput>,
    stats: Option<Stats>,
    position: i32,
    team_type: i32,
    is_sub: bool,
}

impl EntityBuilder {
    pub fn new(hero: HeroBuildInput, position: i32, team_type: i32, is_sub: bool) -> Self {
        Self {
            hero,
            equips: Vec::new(),
            stats: None,
            position,
            team_type,
            is_sub,
        }
    }

    pub fn with_equips(mut self, equips: Vec<EquipmentBuildInput>) -> Self {
        self.equips = equips;
        self
    }

    pub fn with_balance(mut self, balance: BattleBalance, stats: Stats) -> Self {
        let inputs = balance.apply(StatInputs::from_build_input(&self.hero, None));
        self.hero.level = inputs.level;
        self.hero.rank = inputs.rank;
        self.hero.talent = inputs.talent;
        self.hero.talent_style = inputs.talent_style;
        self.hero.talent_placements = inputs.talent_placements;
        for equip in &mut self.equips {
            equip.level = equip.level.max(balance.equip_level);
        }
        self.stats = Some(stats);
        self
    }

    pub fn build(self) -> FightEntityInfo {
        let hero = &self.hero;
        let destiny = Destiny::get(hero.destiny_stone, hero.destiny_rank);
        let attr = self
            .stats
            .map(Stats::base)
            .unwrap_or_else(|| Attr::get(hero, &self.equips));
        let (sg1, sg2) = Skill::get(hero, self.is_sub, destiny.as_ref());
        let ex_point_type = Self::ex_point_type(hero.hero_id);
        let ex_skill = Self::wire_ex_skill(ex_point_type, Skill::get_ex(hero, destiny.as_ref()));
        let passives = Passive::get(hero, &self.equips, destiny.as_ref());
        // Source attribution (Insight/Rank/Destiny/Psychube/Extra) is tracked
        // in `PassiveSkill` for downstream consumers; the wire format only
        // carries raw skill ids.
        let passive_skill_ids = passives.iter().map(|p| p.skill_id).collect();
        let primary_equip_uid = self
            .equips
            .first()
            .map(|equip| equip.uid)
            .unwrap_or_default();
        let equips = self
            .equips
            .iter()
            .map(|equip| EquipRecord {
                equip_uid: Some(equip.uid),
                equip_id: Some(equip.equip_id),
                equip_lv: Some(equip.level),
                refine_lv: Some(equip.refine_level),
            })
            .collect();

        FightEntityInfo {
            uid: Some(hero.uid),
            model_id: Some(hero.hero_id),
            skin: Some(hero.skin),
            position: Some(self.position),
            entity_type: Some(1),
            user_id: Some(hero.user_id),
            ex_point: Some(0),
            level: Some(hero.level),
            current_hp: attr.hp,
            attr: Some(attr),
            base_attr: Some(attr),
            skill_group1: sg1,
            skill_group2: sg2,
            passive_skill: passive_skill_ids,
            ex_skill: Some(ex_skill),
            shield_value: Some(0),
            expoint_max_add: Some(0),
            buff_harm_statistic: Some(0),
            equip_uid: Some(primary_equip_uid),
            trial_equip: Some(EquipRecord::default()),
            ex_skill_level: Some(hero.ex_skill_level),
            power_infos: Self::hero_power_infos(hero.hero_id),
            ex_skill_point_change: Some(0),
            team_type: Some(self.team_type),
            enhance_info_box: Some(EnhanceInfoBox {
                uid: Some(hero.uid),
                can_upgrade_ids: vec![],
                upgraded_options: vec![],
            }),
            trial_id: Some(0),
            career: Some(Self::career(hero.hero_id)),
            status: Some(0),
            guard: Some(-1),
            sub_cd: Some(0),
            ex_point_type: Some(ex_point_type),
            equips,
            destiny_stone: Some(hero.destiny_stone),
            destiny_rank: Some(hero.destiny_rank),
            custom_unit_id: Some(0),
            ..Default::default()
        }
    }

    pub fn trial(
        trial_id: i32,
        uid: i64,
        position: i32,
        team_type: i32,
    ) -> Result<(FightEntityInfo, Stats)> {
        let tables = config::configs::get();
        let trial = tables
            .hero_trial
            .get(trial_id)
            .with_context(|| format!("unknown trial hero {trial_id}"))?;
        let character = tables
            .character
            .get(trial.hero_id)
            .with_context(|| format!("unknown trial character {}", trial.hero_id))?;
        let rank = rank_from_level(trial.hero_id, trial.level);
        let talent = tables
            .character_talent
            .iter()
            .filter(|row| {
                row.hero_id == trial.hero_id
                    && row.talent_id <= trial.talent
                    && row.requirement <= rank
            })
            .map(|row| row.talent_id)
            .max()
            .unwrap_or(1);
        let inputs = StatInputs {
            hero_id: trial.hero_id,
            level: trial.level,
            rank,
            destiny_rank: trial.facetslevel,
            equip_id: trial.equip_id,
            equip_level: trial.equip_lv,
            talent,
            ..Default::default()
        };
        let linked_psychube = tables.linked_psychube_id(trial.hero_id, trial.equip_id);
        let mut stats = Stats::build(&inputs);
        if let Some(equip_id) = linked_psychube {
            stats = stats
                + Stats::equipment_bonus(&StatInputs {
                    equip_id,
                    ..inputs.clone()
                });
        }
        let attr = stats.base();
        let (skill_group1, skill_group2, configured_ex_skill) =
            Skill::for_loadout(trial.hero_id, trial.ex_skill_lv);
        let ex_point_type = Self::ex_point_type(trial.hero_id);
        let ex_skill = Self::wire_ex_skill(ex_point_type, configured_ex_skill);
        let mut passive_skill = Passive::for_ranked_loadout(
            trial.hero_id,
            rank,
            trial.ex_skill_lv,
            (trial.equip_id != 0).then_some((trial.equip_id, trial.equip_refine.max(1))),
            (trial.facets_id != 0).then_some((trial.facets_id, trial.facetslevel)),
        );
        if let Some(equip_id) = linked_psychube {
            passive_skill.extend(Passive::psychube(equip_id, Some(trial.equip_refine.max(1))));
        }
        let passive_skill = passive_skill
            .into_iter()
            .map(|passive| passive.skill_id)
            .collect();

        Ok((
            FightEntityInfo {
                uid: Some(uid),
                model_id: Some(trial.hero_id),
                skin: Some(if trial.skin == 0 {
                    character.skin_id
                } else {
                    trial.skin
                }),
                position: Some(position),
                entity_type: Some(1),
                user_id: Some(0),
                ex_point: Some(0),
                level: Some(trial.level),
                current_hp: attr.hp,
                attr: Some(attr),
                base_attr: Some(attr),
                skill_group1,
                skill_group2,
                passive_skill,
                ex_skill: Some(ex_skill),
                shield_value: Some(0),
                expoint_max_add: Some(0),
                buff_harm_statistic: Some(0),
                equip_uid: Some(0),
                trial_equip: Some(EquipRecord {
                    equip_uid: None,
                    equip_id: Some(trial.equip_id),
                    equip_lv: Some(trial.equip_lv),
                    refine_lv: Some(trial.equip_refine),
                }),
                ex_skill_level: Some(trial.ex_skill_lv),
                power_infos: Self::hero_power_infos(trial.hero_id),
                ex_skill_point_change: Some(0),
                team_type: Some(team_type),
                enhance_info_box: Some(EnhanceInfoBox {
                    uid: Some(uid),
                    ..Default::default()
                }),
                trial_id: Some(trial_id),
                career: Some(character.career),
                status: Some(0),
                guard: Some(-1),
                sub_cd: Some(0),
                ex_point_type: Some(ex_point_type),
                destiny_stone: Some(trial.facets_id),
                destiny_rank: Some(trial.facetslevel),
                custom_unit_id: Some(0),
                ex_point_max: Some(Self::ex_point_max(trial.hero_id)),
                ..Default::default()
            },
            stats,
        ))
    }

    pub fn player(user_id: i64, team_type: i32) -> FightEntityInfo {
        let uid = if team_type == 1 { 0 } else { -99999 };
        let attr = HeroAttribute {
            hp: Some(100),
            attack: Some(0),
            defense: Some(0),
            mdefense: Some(0),
            technic: Some(0),
            multi_hp_idx: Some(0),
            multi_hp_num: Some(0),
        };

        FightEntityInfo {
            uid: Some(uid),
            model_id: Some(0),
            skin: Some(0),
            position: Some(0),
            entity_type: Some(3),
            user_id: Some(user_id),
            ex_point: Some(0),
            level: Some(0),
            current_hp: Some(100),
            attr: Some(attr),
            base_attr: Some(attr),
            ex_skill: Some(0),
            shield_value: Some(0),
            expoint_max_add: Some(0),
            buff_harm_statistic: Some(0),
            equip_uid: Some(0),
            ex_skill_level: Some(0),
            ex_skill_point_change: Some(0),
            team_type: Some(team_type),
            enhance_info_box: Some(EnhanceInfoBox {
                uid: Some(uid),
                can_upgrade_ids: vec![],
                upgraded_options: vec![],
            }),
            trial_id: Some(0),
            career: Some(0),
            status: Some(0),
            guard: Some(-1),
            sub_cd: Some(0),
            ex_point_type: Some(0),
            destiny_stone: Some(0),
            destiny_rank: Some(0),
            custom_unit_id: Some(0),
            ..Default::default()
        }
    }

    fn ex_point_type(hero_id: i32) -> i32 {
        Self::ex_point_spec(hero_id).0
    }

    fn ex_point_max(hero_id: i32) -> i32 {
        Self::ex_point_spec(hero_id).1
    }

    fn wire_ex_skill(ex_point_type: i32, configured: i32) -> i32 {
        if ExPointKind::from_wire(ex_point_type) == ExPointKind::DevicePower {
            0
        } else {
            configured
        }
    }

    fn ex_point_spec(hero_id: i32) -> (i32, i32) {
        let game = config::configs::get();
        let spec = game
            .character_rank_replace
            .get(hero_id)
            .map(|r| r.unique_skill_point.as_str())
            .or_else(|| {
                game.character
                    .get(hero_id)
                    .map(|c| c.unique_skill_point.as_str())
            });

        let mut values = spec.into_iter().flat_map(|spec| spec.split('#'));
        (
            values
                .next()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_default(),
            values
                .next()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_default(),
        )
    }

    fn career(hero_id: i32) -> i32 {
        config::configs::get()
            .character
            .get(hero_id)
            .map(|c| c.career)
            .unwrap_or(0)
    }

    fn hero_power_infos(hero_id: i32) -> Vec<PowerInfo> {
        config::configs::get()
            .character
            .get(hero_id)
            .into_iter()
            .flat_map(|c| parse_power_specs(&c.power_max))
            .filter(|(_, max)| *max > 0)
            .map(|(power_id, max)| PowerInfo {
                power_id: Some(power_id),
                num: Some(0),
                max: Some(max),
            })
            .collect()
    }
}

fn parse_power_specs(spec: &str) -> Vec<(i32, i32)> {
    spec.split('|')
        .filter_map(|entry| {
            let (power_id, max) = entry.trim().split_once('#')?;
            Some((power_id.parse().ok()?, max.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::EntityBuilder;

    #[test]
    fn device_power_trial_keeps_its_unique_skill_out_of_ex_skill() {
        crate::test_support::init_config();

        let (entity, _) = EntityBuilder::trial(116385001, 10, 1, 1).unwrap();

        assert_eq!(entity.model_id, Some(3149));
        assert_eq!(entity.ex_point_type, Some(4));
        assert_eq!(entity.ex_point_max, Some(100));
        assert_eq!(entity.ex_skill, Some(0));
    }
}
