use anyhow::{Context, Result};
use database::{db::game::equipment::Equipment, models::game::heros::HeroData};
use sonettobuf::{EnhanceInfoBox, EquipRecord, FightEntityInfo, HeroAttribute, PowerInfo};

use super::{
    attr::Attr,
    destiny::Destiny,
    passive::Passive,
    skill::Skill,
    stats::{StatInputs, Stats, rank_from_level},
};

pub struct EntityBuilder {
    hero_data: HeroData,
    equips: Vec<Equipment>,
    position: i32,
    team_type: i32,
    is_sub: bool,
}

impl EntityBuilder {
    pub fn new(hero_data: HeroData, position: i32, team_type: i32, is_sub: bool) -> Self {
        Self {
            hero_data,
            equips: Vec::new(),
            position,
            team_type,
            is_sub,
        }
    }

    pub fn with_equips(mut self, equips: Vec<Equipment>) -> Self {
        self.equips = equips;
        self
    }

    pub fn build(self) -> FightEntityInfo {
        let r = &self.hero_data.record;
        let destiny = Destiny::get(r.destiny_stone, r.destiny_rank);
        let attr = Attr::get(&self.hero_data, &self.equips);
        let (sg1, sg2) = Skill::get(&self.hero_data, self.is_sub, destiny.as_ref());
        let passives = Passive::get(&self.hero_data, &self.equips, destiny.as_ref());
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
                refine_lv: Some(equip.refine_lv),
            })
            .collect();

        FightEntityInfo {
            uid: Some(r.uid),
            model_id: Some(r.hero_id),
            skin: Some(r.skin),
            position: Some(self.position),
            entity_type: Some(1),
            user_id: Some(r.user_id),
            ex_point: Some(0),
            level: Some(r.level),
            current_hp: attr.hp,
            attr: Some(attr),
            base_attr: Some(attr),
            skill_group1: sg1,
            skill_group2: sg2,
            passive_skill: passive_skill_ids,
            ex_skill: Some(Skill::get_ex(&self.hero_data, destiny.as_ref())),
            shield_value: Some(0),
            expoint_max_add: Some(0),
            buff_harm_statistic: Some(0),
            equip_uid: Some(primary_equip_uid),
            trial_equip: Some(EquipRecord::default()),
            ex_skill_level: Some(r.ex_skill_level),
            power_infos: Self::hero_power_infos(r.hero_id),
            ex_skill_point_change: Some(0),
            team_type: Some(self.team_type),
            enhance_info_box: Some(EnhanceInfoBox {
                uid: Some(r.uid),
                can_upgrade_ids: vec![],
                upgraded_options: vec![],
            }),
            trial_id: Some(0),
            career: Some(Self::career(r.hero_id)),
            status: Some(0),
            guard: Some(-1),
            sub_cd: Some(0),
            ex_point_type: Some(Self::ex_point_type(r.hero_id)),
            equips,
            destiny_stone: Some(r.destiny_stone),
            destiny_rank: Some(r.destiny_rank),
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
        let (skill_group1, skill_group2, ex_skill) =
            Skill::for_loadout(trial.hero_id, trial.ex_skill_lv);
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
                ex_point_type: Some(Self::ex_point_type(trial.hero_id)),
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
