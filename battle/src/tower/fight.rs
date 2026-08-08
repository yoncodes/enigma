use std::collections::HashSet;

use crate::{
    dungeon::BuiltFight,
    engine::{
        entity::attr::AttrId,
        fight::rules::{ATTACKER_SIDE_UID, OwnedBattleSkill},
        manager::eureka::PowerType,
    },
};
use database::db::game::tower as tower_db;
use database::models::game::tower::{TowerConstId, TowerType};
use sonettobuf::{
    AssistBossInfo, AssistBossSkillInfo, EnhanceInfoBox, EquipRecord, FightEntityInfo, FightGroup,
    HeroAttribute, HeroExAttribute, PowerInfo,
};
use sqlx::SqlitePool;

use super::BattleContext;
use crate::dungeon::FightOptions;

const ASSIST_BOSS_UID: i64 = -1;

pub async fn build_fight(
    pool: &SqlitePool,
    player_id: i64,
    episode_id: i32,
    battle_id: i32,
    fight_group: &FightGroup,
    options: FightOptions,
    context: BattleContext,
) -> anyhow::Result<BuiltFight> {
    let mut built = crate::dungeon::build_fight(
        pool,
        player_id,
        episode_id,
        battle_id,
        fight_group,
        options,
        None,
    )
    .await?;
    let boss_id = fight_group.assist_boss_id.unwrap_or_default();
    if boss_id == 0 {
        return Ok(built);
    }

    let tables = config::configs::get();
    let owned_level = tower_db::assist_boss_level(pool, player_id, boss_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("assist boss {boss_id} is not owned"))?;
    let boss_level = effective_level(tables, context, owned_level);
    let talent_ids = if let Some(plan) = tables
        .tower_talent_plan
        .iter()
        .find(|plan| plan.boss_id == boss_id && plan.plan_id == context.talent_plan_id)
    {
        system_plan_talents(tables, boss_id, boss_level, &plan.talent_ids)
    } else {
        tower_db::talent_plan_ids(pool, player_id, boss_id, context.talent_plan_id).await?
    };

    apply_assist_boss(
        tables,
        player_id,
        boss_id,
        boss_level,
        &talent_ids,
        &mut built,
    )?;
    Ok(built)
}

fn effective_level(tables: &config::GameDB, context: BattleContext, owned_level: i32) -> i32 {
    match context.tower_type {
        value if value == TowerType::Boss.id() && context.layer_id == 0 => {
            tower_const(tables, TowerConstId::TeachBossLevel).unwrap_or(owned_level)
        }
        value if value == TowerType::Limited.id() => owned_level
            .max(tower_const(tables, TowerConstId::BalanceBossLevel).unwrap_or(owned_level)),
        _ => owned_level,
    }
}

fn tower_const(tables: &config::GameDB, id: TowerConstId) -> Option<i32> {
    tables.tower_const.get(id.id())?.value.parse().ok()
}

pub(super) fn system_plan_talents(
    tables: &config::GameDB,
    boss_id: i32,
    boss_level: i32,
    configured_ids: &str,
) -> Vec<i32> {
    let budget: i32 = tables
        .tower_assist_develop
        .iter()
        .filter(|row| row.boss_id == boss_id && row.level <= boss_level)
        .map(|row| row.talent_point)
        .sum();
    let mut spent = 0;
    configured_ids
        .split('#')
        .filter_map(|value| value.parse().ok())
        .map_while(|id| {
            let talent = tables
                .tower_assist_talent
                .iter()
                .find(|row| row.boss_id == boss_id && row.node_id == id)?;
            spent += talent.consume;
            (spent <= budget).then_some(id)
        })
        .collect()
}

pub fn system_plan_rule_skills(
    tables: &config::GameDB,
    fight: &sonettobuf::Fight,
    plan_id: i32,
) -> Vec<OwnedBattleSkill> {
    let Some(assist_boss) = fight
        .attacker
        .as_ref()
        .and_then(|team| team.assist_boss.as_ref())
    else {
        return Vec::new();
    };
    let boss_id = assist_boss.model_id.unwrap_or_default();
    let boss_level = assist_boss.level.unwrap_or_default();
    let Some(plan) = tables
        .tower_talent_plan
        .iter()
        .find(|plan| plan.boss_id == boss_id && plan.plan_id == plan_id)
    else {
        return Vec::new();
    };
    let talent_ids = system_plan_talents(tables, boss_id, boss_level, &plan.talent_ids);
    let active_talents = talent_ids.into_iter().collect::<HashSet<_>>();
    let values = tables
        .tower_assist_develop
        .iter()
        .filter(|row| row.boss_id == boss_id && row.level <= boss_level)
        .map(|row| row.extra_rule.as_str())
        .chain(
            tables
                .tower_assist_talent
                .iter()
                .filter(|row| row.boss_id == boss_id && active_talents.contains(&row.node_id))
                .map(|row| row.extra_rule.as_str()),
        );
    let mut skills = parsed_extra_rules(values)
        .filter(|(kind, _)| *kind == 1)
        .map(|(_, skill_id)| OwnedBattleSkill {
            owner_uid: ATTACKER_SIDE_UID,
            skill_id,
        })
        .collect::<Vec<_>>();
    skills.sort_unstable_by_key(|skill| (skill.owner_uid, skill.skill_id));
    skills.dedup();
    skills
}

pub(super) fn apply_assist_boss(
    tables: &config::GameDB,
    player_id: i64,
    boss_id: i32,
    boss_level: i32,
    talent_ids: &[i32],
    built: &mut BuiltFight,
) -> anyhow::Result<()> {
    let boss = tables
        .tower_assist_boss
        .iter()
        .find(|row| row.boss_id == boss_id)
        .ok_or_else(|| anyhow::anyhow!("unknown assist boss {boss_id}"))?;
    let active_talents = talent_ids.iter().copied().collect::<HashSet<_>>();
    let talents = tables
        .tower_assist_talent
        .iter()
        .filter(|row| row.boss_id == boss_id && active_talents.contains(&row.node_id))
        .collect::<Vec<_>>();
    let extra_rules = tables
        .tower_assist_develop
        .iter()
        .filter(|row| row.boss_id == boss_id && row.level <= boss_level)
        .map(|row| row.extra_rule.as_str())
        .chain(talents.iter().map(|talent| talent.extra_rule.as_str()))
        .collect::<Vec<_>>();
    apply_extra_rules(&extra_rules, built)?;
    let attacker = built
        .fight
        .attacker
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("fight has no attacker team"))?;
    let hero_count = attacker.entitys.len();
    if hero_count == 0 {
        anyhow::bail!("tower fight has no attacker entities");
    }
    let team_level = attacker
        .entitys
        .iter()
        .filter_map(|entity| entity.level)
        .sum::<i32>()
        / hero_count as i32;
    let base = tables
        .tower_assist_attribute
        .iter()
        .find(|row| row.boss_id == boss_id && row.team_level == team_level)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "missing assist boss attributes for boss {boss_id} and team level {team_level}"
            )
        })?;
    let attack_rate: i32 = tables
        .tower_assist_develop
        .iter()
        .filter(|row| row.boss_id == boss_id && row.level <= boss_level)
        .flat_map(|row| pairs(&row.attribute))
        .filter(|(id, _)| AttrId::from_raw(*id) == Some(AttrId::Attack))
        .map(|(_, value)| value)
        .sum();
    let attr = HeroAttribute {
        hp: Some(base.hp),
        attack: Some(base.attack * (1000 + attack_rate) / 1000),
        defense: Some(0),
        mdefense: Some(0),
        technic: Some(0),
        multi_hp_idx: Some(0),
        multi_hp_num: Some(0),
    };
    for hero in attacker.entitys.iter_mut().chain(&mut attacker.sub_entitys) {
        let career = hero.career.unwrap_or_default();
        hero.passive_skill.extend(
            talents
                .iter()
                .filter_map(|talent| career_passive(&talent.hero_passive_skills, career)),
        );
    }

    let mut passives = ids(&boss.passive_skills);
    for row in tables
        .tower_assist_develop
        .iter()
        .filter(|row| row.boss_id == boss_id && row.level <= boss_level)
    {
        passives.extend(ids(&row.passive_skills));
    }
    passives.extend(
        talents
            .iter()
            .flat_map(|talent| ids(&talent.boss_passive_skills)),
    );
    passives.extend(ids(&boss.teach_skills));

    attacker.assist_boss = Some(FightEntityInfo {
        uid: Some(ASSIST_BOSS_UID),
        model_id: Some(boss_id),
        skin: Some(boss.skin_id),
        position: Some(0),
        entity_type: Some(5),
        user_id: Some(player_id),
        ex_point: Some(0),
        level: Some(boss_level),
        current_hp: Some(base.hp),
        attr: Some(attr),
        passive_skill: passives,
        ex_skill: Some(0),
        shield_value: Some(0),
        expoint_max_add: Some(0),
        buff_harm_statistic: Some(0),
        equip_uid: Some(0),
        trial_equip: Some(EquipRecord::default()),
        ex_skill_level: Some(0),
        power_infos: vec![PowerInfo {
            power_id: Some(PowerType::AssistBoss.id()),
            num: Some(boss.res_init_val),
            max: Some(boss.res_max_val),
        }],
        base_attr: Some(attr),
        ex_skill_point_change: Some(0),
        team_type: Some(1),
        enhance_info_box: Some(EnhanceInfoBox {
            uid: Some(ASSIST_BOSS_UID),
            ..Default::default()
        }),
        trial_id: Some(0),
        career: Some(boss.career),
        status: Some(0),
        guard: Some(-1),
        sub_cd: Some(0),
        ex_point_type: Some(0),
        destiny_stone: Some(0),
        destiny_rank: Some(0),
        custom_unit_id: Some(0),
        ..Default::default()
    });
    attacker.assist_boss_info = Some(AssistBossInfo {
        skills: active_skills(&boss.active_skills),
        curr_cd: Some(0),
        cd_cfg: Some(boss.cold_time),
        form_id: Some(0),
        round_use_limit: Some(1),
        exceed_use_free: Some(0),
        params: None,
        r#type: Some(0),
    });
    built.ex_attributes.push((
        ASSIST_BOSS_UID,
        HeroExAttribute {
            cri: Some(base.cri),
            cri_dmg: Some(base.cri_dmg),
            ..Default::default()
        },
    ));
    Ok(())
}

fn ids(value: &str) -> Vec<i32> {
    value
        .split(['#', '|'])
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn pairs(value: &str) -> impl Iterator<Item = (i32, i32)> + '_ {
    value.split('|').filter_map(|pair| {
        let (id, value) = pair.split_once('#')?;
        Some((id.parse().ok()?, value.parse().ok()?))
    })
}

fn apply_extra_rules(values: &[&str], built: &mut BuiltFight) -> anyhow::Result<()> {
    for (kind, skill_id) in parsed_extra_rules(values.iter().copied()) {
        match kind {
            1 => built.battle_rule_skills.push(OwnedBattleSkill {
                owner_uid: ATTACKER_SIDE_UID,
                skill_id,
            }),
            2 => {
                let defender = built
                    .fight
                    .defender
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("tower fight has no defender team"))?;
                for entity in defender.entitys.iter_mut().chain(&mut defender.sub_entitys) {
                    entity.passive_skill.push(skill_id);
                }
            }
            _ => anyhow::bail!("unsupported tower extra-rule kind {kind}"),
        }
    }
    Ok(())
}

fn parsed_extra_rules<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> impl Iterator<Item = (i32, i32)> {
    values.into_iter().flat_map(|value| {
        value.split('|').filter_map(|rule| {
            let (kind, skill_id) = rule.split_once('#')?;
            Some((kind.parse::<i32>().ok()?, skill_id.parse::<i32>().ok()?))
        })
    })
}

fn career_passive(value: &str, career: i32) -> Option<i32> {
    value.split('|').find_map(|entry| {
        let (target_career, skill_id) = entry.split_once(':')?;
        (target_career.parse::<i32>().ok()? == career)
            .then(|| skill_id.parse().ok())
            .flatten()
    })
}

fn active_skills(value: &str) -> Vec<AssistBossSkillInfo> {
    value
        .split('|')
        .filter_map(|entry| {
            let values = entry
                .split('#')
                .map(str::parse::<i32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            Some(AssistBossSkillInfo {
                skill_id: values.first().copied(),
                need_power: values.get(1).copied(),
                power_low: values.get(2).copied(),
                power_high: values.get(3).copied(),
            })
        })
        .collect()
}
