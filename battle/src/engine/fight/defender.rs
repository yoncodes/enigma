use anyhow::Result;
use sonettobuf::{EquipRecord, FightEntityInfo, FightTeam, PowerInfo};

use super::team::Team;
use crate::engine::entity::{builder::EntityBuilder, skill::parse_skill_group};

pub struct Defender;

pub struct DefenderSetup {
    pub max_round: i32,
    pub team: FightTeam,
    pub attacker_sp_entitys: Vec<FightEntityInfo>,
    pub attacker_sp_fight_entities: Vec<FightEntityInfo>,
}

impl Defender {
    pub fn get(battle_id: i32, uid_offset: usize) -> Result<DefenderSetup> {
        let game_data = config::configs::get();
        let battle = game_data
            .battle
            .get(battle_id)
            .ok_or_else(|| anyhow::anyhow!("Battle {} not found", battle_id))?;

        let monster_max = battle.monster_max as usize;
        let group_id = battle
            .monster_group_ids
            .split('#')
            .next()
            .and_then(|id| id.parse::<i32>().ok())
            .ok_or_else(|| anyhow::anyhow!("No monster group in battle {}", battle_id))?;
        let group = game_data
            .monster_group
            .get(group_id)
            .ok_or_else(|| anyhow::anyhow!("MonsterGroup {} not found", group_id))?;
        let (entitys, sub_entitys) =
            Self::build_wave_entities(group_id, monster_max, 2, uid_offset)?;
        let mut next_uid_index = uid_offset + monster_max;
        let mut build_specials = |raw: &str, team_type: i32| -> Result<Vec<FightEntityInfo>> {
            monster_ids(raw)
                .into_iter()
                .enumerate()
                .map(|(index, monster_id)| {
                    let entity = Self::build_enemy(
                        monster_id,
                        next_uid_index,
                        (monster_max + index + 1) as i32,
                        team_type,
                    )?;
                    next_uid_index += 1;
                    Ok(entity)
                })
                .collect()
        };
        let defender_sp_entitys = build_specials(&group.sp_monster, 2)?;
        let attacker_sp_entitys = build_specials(&group.sp_supporter, 1)?;
        let defender_sp_fight_entities = build_specials(&group.sp2_monster, 2)?;
        let attacker_sp_fight_entities = build_specials(&group.sp2_supporter, 1)?;
        let mut team = Team::build(
            entitys,
            sub_entitys,
            EntityBuilder::player(0, 2),
            Some(0),
            Some(0),
            vec![],
        );
        team.sp_entitys = defender_sp_entitys;
        team.sp_fight_entities = defender_sp_fight_entities;
        Ok(DefenderSetup {
            max_round: battle.max_round,
            team,
            attacker_sp_entitys,
            attacker_sp_fight_entities,
        })
    }

    pub(crate) fn build_wave_entities(
        group_id: i32,
        monster_max: usize,
        team_type: i32,
        uid_offset: usize,
    ) -> Result<(Vec<FightEntityInfo>, Vec<FightEntityInfo>)> {
        let game_data = config::configs::get();
        let group = game_data
            .monster_group
            .get(group_id)
            .ok_or_else(|| anyhow::anyhow!("MonsterGroup {} not found", group_id))?;
        let monster_ids = monster_ids(&group.monster);

        let mut entitys = Vec::new();
        for (idx, monster_id) in monster_ids.iter().take(monster_max).enumerate() {
            entitys.push(Self::build_enemy(
                *monster_id,
                uid_offset + idx,
                (idx + 1) as i32,
                team_type,
            )?);
        }

        let mut sub_entitys = Vec::new();
        for (i, monster_id) in monster_ids.iter().skip(monster_max).enumerate() {
            let idx = monster_max + i;
            sub_entitys.push(Self::build_enemy(
                *monster_id,
                uid_offset + idx,
                -((i + 1) as i32),
                team_type,
            )?);
        }

        Ok((entitys, sub_entitys))
    }

    fn build_enemy(
        monster_id: i32,
        idx: usize,
        position: i32,
        team_type: i32,
    ) -> Result<FightEntityInfo> {
        let uid = -((idx + 1) as i64);
        Self::build_monster_with_uid(monster_id, uid, position, team_type)
    }

    pub(crate) fn build_monster_with_uid(
        monster_id: i32,
        uid: i64,
        position: i32,
        team_type: i32,
    ) -> Result<FightEntityInfo> {
        let game_data = config::configs::get();
        let monster = game_data
            .monster
            .get(monster_id)
            .ok_or_else(|| anyhow::anyhow!("Monster {} not found", monster_id))?;
        let skill_template = game_data
            .monster_skill_template
            .get(monster.skill_template)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Monster skill template {} not found",
                    monster.skill_template
                )
            })?;
        let level = if monster.level_true != 0 {
            monster.level_true
        } else {
            monster.level
        };

        let stats = crate::engine::entity::stats::monster_stats(monster_id, level)
            .ok_or_else(|| anyhow::anyhow!("Monster stats {} not found", monster_id))?;
        let attr = stats.base();
        let (toughness_value, toughness_point) = crate::engine::manager::toughness::initial_values(
            &monster.toughness,
            attr.hp.unwrap_or_default(),
        )
        .map(|(value, point)| (Some(value), Some(point)))
        .unwrap_or_default();

        Ok(FightEntityInfo {
            uid: Some(uid),
            model_id: Some(monster.id),
            skin: Some(monster.skin_id),
            position: Some(position),
            entity_type: Some(2),
            user_id: Some(0),
            ex_point: Some(monster.initial_unique_skill_point),
            level: Some(level),
            current_hp: attr.hp,
            attr: Some(attr),
            base_attr: Some(attr),
            skill_group1: parse_skill_group(&skill_template.active_skill, 1),
            skill_group2: parse_skill_group(&skill_template.active_skill, 2),
            passive_skill: parse_passives(&skill_template.passive_skill)
                .into_iter()
                .take(monster.passive_skill_count.max(0) as usize)
                .chain(parse_passives(&monster.passive_skills_ex))
                .chain(game_data.toughness_passive_skills(monster.toughness_skill))
                .collect(),
            ex_skill: Some(
                skill_template
                    .unique_skill
                    .split('#')
                    .next()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0),
            ),
            shield_value: Some(0),
            expoint_max_add: Some(0),
            buff_harm_statistic: Some(0),
            equip_uid: Some(0),
            trial_equip: Some(EquipRecord::default()),
            ex_skill_level: Some(0),
            power_infos: monster_power_infos(&skill_template.power_max),
            ex_skill_point_change: Some(0),
            team_type: Some(team_type),
            enhance_info_box: Some(sonettobuf::EnhanceInfoBox {
                uid: Some(uid),
                can_upgrade_ids: vec![],
                upgraded_options: vec![],
            }),
            trial_id: Some(0),
            career: Some(skill_template.career),
            status: Some(0),
            guard: Some(-1),
            sub_cd: Some(0),
            ex_point_type: Some(0),
            ex_point_max: Some(skill_template.unique_skill_point),
            destiny_stone: Some(0),
            destiny_rank: Some(0),
            custom_unit_id: Some(0),
            weak_careers: monster_ids(&monster.career_weak),
            toughness_value,
            toughness_point,
            is_broken: Some(false),
            ..Default::default()
        })
    }
}

pub(crate) fn monster_ids(value: &str) -> Vec<i32> {
    value
        .split('#')
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn parse_passives(value: &str) -> Vec<i32> {
    value
        .split(['#', '|'])
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect()
}

fn monster_power_infos(value: &str) -> Vec<PowerInfo> {
    let mut parts = value.split('#');
    let power_id = parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let max = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    if max <= 0 {
        return vec![];
    }
    vec![PowerInfo {
        power_id: Some(power_id),
        num: Some(0),
        max: Some(max),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unseeded_boss_uses_instance_job_scaling() {
        crate::test_support::init_config();

        let boss = Defender::build_monster_with_uid(30111001, -1, 1, 2).unwrap();
        let attr = boss.attr.unwrap();

        assert_eq!(boss.current_hp, Some(67_680));
        assert_eq!(attr.hp, Some(67_680));
        assert_eq!(attr.attack, Some(1_696));
        assert_eq!(attr.defense, Some(1_000));
        assert_eq!(attr.mdefense, Some(736));
        assert_eq!(attr.technic, Some(210));
    }

    #[test]
    fn monster_toughness_skill_contributes_its_configured_passive() {
        crate::test_support::init_config();

        let entity = Defender::build_monster_with_uid(1_163_857_113, -1, 1, 2).unwrap();

        assert!(entity.passive_skill.contains(&116_362_200));
    }

    #[test]
    fn monster_unique_skill_level_is_not_serialized_as_a_hero_ex_rank() {
        crate::test_support::init_config();

        let monster = Defender::build_monster_with_uid(4_030_703, -3, 1, 2).unwrap();

        assert_eq!(monster.ex_skill, Some(40_231_331));
        assert_eq!(monster.ex_skill_level, Some(0));
    }

    #[test]
    fn monster_uses_its_configured_moxie_maximum() {
        crate::test_support::init_config();

        let monster = Defender::build_monster_with_uid(109_360_002, -1, 1, 2).unwrap();

        assert_eq!(monster.ex_point_max, Some(2));
    }

    #[test]
    fn monster_starts_with_configured_moxie() {
        crate::test_support::init_config();

        let setup = Defender::get(1001, 2).unwrap();
        let monster = setup
            .team
            .entitys
            .iter()
            .find(|entity| entity.model_id == Some(100104))
            .unwrap();

        assert_eq!(monster.ex_point, Some(5));
    }

    #[test]
    fn tower_supporter_uses_reserved_normal_uid_space() {
        crate::test_support::init_config();

        let setup = Defender::get(9000303, 1).unwrap();

        assert_eq!(
            setup
                .team
                .entitys
                .iter()
                .filter_map(|entity| entity.uid)
                .collect::<Vec<_>>(),
            vec![-2, -3, -4]
        );
        assert_eq!(setup.attacker_sp_entitys[0].uid, Some(-6));
        assert_eq!(setup.attacker_sp_entitys[0].model_id, Some(900030304));
    }
}
