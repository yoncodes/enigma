use crate::engine::{
    entity::{
        builder::EntityBuilder,
        input::{EquipmentBuildInput, HeroBuildInput},
        stats::{BattleBalance, StatInputs, Stats},
    },
    fight::{defender::Defender, team::Team},
    manager::eureka::PowerType,
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sonettobuf::{
    AssistBossInfo, AssistBossSkillInfo, EnhanceInfoBox, EquipRecord, FightEntityInfo, FightTeam,
    HeroExAttribute, HeroSpAttribute, PowerInfo,
};
use std::collections::{HashMap, HashSet};

const SUPPORT_UID: i64 = -1;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComposeParams {
    theme_id: i32,
    layer_id: i32,
    plane_id: i32,
    support_id: i32,
    #[serde(default)]
    support_assist_uid: i64,
    #[serde(default)]
    support_assist_type: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComposeGroupParams {
    buff_params_str: String,
    #[serde(default)]
    assist_data_map: Vec<Option<HashMap<String, ComposeAssist>>>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComposeAssist {
    hero_uid: i64,
    hero_id: i32,
    level: i32,
    skin: i32,
    ex_skill_level: i32,
    assist_type: i32,
}

pub struct Attacker;

#[derive(Clone, Debug, Default)]
pub struct BattleFighter {
    pub hero: HeroBuildInput,
    pub equips: Vec<EquipmentBuildInput>,
}

#[derive(Clone, Debug, Default)]
pub struct BattleRoster {
    pub user_id: i64,
    pub player_level: i32,
    pub fighters: Vec<BattleFighter>,
    pub compose_support: Option<HeroBuildInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComposeSupportLookup {
    pub hero_id: i32,
    pub hero_uid: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct BattleRosterPlan {
    hero_uids: Vec<i64>,
    compose_support: Option<ComposeSupportLookup>,
}

impl BattleRosterPlan {
    pub fn configured(
        catalog: crate::catalog::BattleCatalog,
        episode_id: i32,
        battle_id: i32,
        is_balance: bool,
        fight_group: &sonettobuf::FightGroup,
        params: Option<&str>,
    ) -> Result<Self> {
        let setup = attacker_setup(
            catalog,
            episode_id,
            battle_id,
            is_balance,
            fight_group,
            params,
        )?;
        Ok(Self {
            hero_uids: fight_group
                .hero_list
                .iter()
                .chain(&fight_group.sub_hero_list)
                .copied()
                .filter(|uid| *uid > 0)
                .collect(),
            compose_support: setup.compose_support.map(|plan| ComposeSupportLookup {
                hero_id: plan.hero_id,
                hero_uid: plan.assist.map(|assist| assist.hero_uid),
            }),
        })
    }

    pub fn hero_uids(&self) -> &[i64] {
        &self.hero_uids
    }

    pub fn compose_support(&self) -> Option<ComposeSupportLookup> {
        self.compose_support
    }
}

struct AttackerSetup {
    role_num: i32,
    balance: Option<BattleBalance>,
    aid_ids: Vec<i32>,
    selected_trials: Vec<(i32, i32)>,
    use_configured_aids: bool,
    compose_support: Option<ComposeSupportPlan>,
}

struct ComposeSupportPlan {
    support_id: i32,
    hero_id: i32,
    assist: Option<ComposeAssist>,
}

pub fn plan_roster(
    episode_id: i32,
    battle_id: i32,
    is_balance: bool,
    fight_group: &sonettobuf::FightGroup,
    params: Option<&str>,
) -> Result<BattleRosterPlan> {
    BattleRosterPlan::configured(
        crate::catalog::BattleCatalog::global(),
        episode_id,
        battle_id,
        is_balance,
        fight_group,
        params,
    )
}

impl BattleRoster {
    fn fighter(&self, uid: i64) -> Result<&BattleFighter> {
        self.fighters
            .iter()
            .find(|fighter| fighter.hero.uid == uid)
            .ok_or_else(|| anyhow::anyhow!("battle roster has no hero uid {uid}"))
    }
}

pub struct BuiltAttacker {
    pub team: FightTeam,
    pub ex_attributes: Vec<(i64, HeroExAttribute)>,
    pub sp_attributes: Vec<(i64, HeroSpAttribute)>,
    pub reserved_uid_offset: usize,
}

impl Attacker {
    pub fn get(
        catalog: crate::catalog::BattleCatalog,
        roster: &BattleRoster,
        episode_id: i32,
        battle_id: i32,
        is_balance: bool,
        fight_group: &sonettobuf::FightGroup,
        params: Option<&str>,
    ) -> Result<BuiltAttacker> {
        let setup = attacker_setup(
            catalog,
            episode_id,
            battle_id,
            is_balance,
            fight_group,
            params,
        )?;
        let balance = setup.balance;
        let aid_ids = setup.aid_ids;
        let selected_trials = setup.selected_trials;
        let use_configured_aids = setup.use_configured_aids;
        let game = catalog.game_data();

        let mut entitys = Vec::new();
        let mut sub_entitys = Vec::new();
        let mut ex_attributes = Vec::new();
        let mut sp_attributes = Vec::new();
        if use_configured_aids {
            for (index, monster_id) in aid_ids.iter().copied().enumerate() {
                let uid = -i64::try_from(index + 1)?;
                entitys.push(Defender::build_monster(
                    catalog,
                    monster_id,
                    uid,
                    (index + 1) as i32,
                    1,
                )?);
            }
        }

        for (index, (trial_id, position)) in selected_trials.iter().copied().enumerate() {
            let uid = -i64::try_from(aid_ids.len() + index + 1)?;
            let (entity, stats) =
                EntityBuilder::configured_trial(catalog, trial_id, uid, position, 1)?;
            ex_attributes.push((uid, stats.ex()));
            sp_attributes.push((uid, stats.sp()));
            if position > 0 {
                entitys.push(entity);
            } else {
                sub_entitys.push(entity);
            }
        }

        let trial_positions = selected_trials
            .iter()
            .filter_map(|(_, position)| (*position > 0).then_some(*position))
            .collect::<HashSet<_>>();
        let mut positions =
            (1..=setup.role_num).filter(|position| !trial_positions.contains(position));
        for hero_uid in &fight_group.hero_list {
            let position = positions
                .next()
                .ok_or_else(|| anyhow::anyhow!("main roster exceeds configured battle slots"))?;
            if *hero_uid == 0 {
                continue;
            }
            if let Some(entity) = configured_aid(catalog, &aid_ids, *hero_uid, position)? {
                entitys.push(entity);
                continue;
            }

            let fighter = roster.fighter(*hero_uid)?;
            let hero_input = fighter.hero.clone();
            let equip_inputs = fighter.equips.clone();
            let stats = balance
                .map(|balance| balance.stats(game, &hero_input, &equip_inputs))
                .unwrap_or_else(|| Stats::loadout(game, &hero_input, &equip_inputs));
            ex_attributes.push((hero_input.uid, stats.ex()));
            sp_attributes.push((hero_input.uid, stats.sp()));

            let mut builder = EntityBuilder::new(hero_input, position, 1)
                .with_catalog(catalog)
                .with_equips(equip_inputs)
                .with_stats(stats);
            if let Some(balance) = balance {
                builder = builder.with_balance(balance, stats);
            }
            entitys.push(builder.build()?);
        }

        for hero_uid in fight_group.sub_hero_list.iter() {
            if *hero_uid == 0 {
                continue;
            }
            if let Some(entity) = configured_aid(catalog, &aid_ids, *hero_uid, -1)? {
                sub_entitys.push(entity);
                continue;
            }

            let fighter = roster.fighter(*hero_uid)?;
            let hero_input = fighter.hero.clone();
            let equip_inputs = fighter.equips.clone();
            let stats = balance
                .map(|balance| balance.stats(game, &hero_input, &equip_inputs))
                .unwrap_or_else(|| Stats::loadout(game, &hero_input, &equip_inputs));
            ex_attributes.push((hero_input.uid, stats.ex()));
            sp_attributes.push((hero_input.uid, stats.sp()));

            let mut builder = EntityBuilder::new(hero_input, -1, 1)
                .with_catalog(catalog)
                .with_equips(equip_inputs)
                .with_stats(stats);
            if let Some(balance) = balance {
                builder = builder.with_balance(balance, stats);
            }
            sub_entitys.push(builder.build()?);
        }

        let player_entity = EntityBuilder::player(roster.user_id, 1);
        let skill_infos = Team::player_skills(catalog, fight_group.cloth_id);

        let mut team = Team::build(
            entitys,
            sub_entitys,
            player_entity,
            Some(15),
            fight_group.cloth_id,
            skill_infos,
        );
        if let Some((support, info, ex, sp)) =
            Self::compose_support(catalog, roster, setup.compose_support.as_ref())?
        {
            team.assist_boss = Some(support);
            team.assist_boss_info = Some(info);
            ex_attributes.push((SUPPORT_UID, ex));
            sp_attributes.push((SUPPORT_UID, sp));
        }
        let reserved_uid_offset = reserved_uid_offset(
            fight_group,
            if use_configured_aids {
                aid_ids.len() + selected_trials.len()
            } else {
                selected_trials.len()
            },
            team.assist_boss.is_some() || fight_group.assist_boss_id.unwrap_or_default() != 0,
        )?;

        Ok(BuiltAttacker {
            team,
            ex_attributes,
            sp_attributes,
            reserved_uid_offset,
        })
    }

    fn compose_support(
        catalog: crate::catalog::BattleCatalog,
        roster: &BattleRoster,
        plan: Option<&ComposeSupportPlan>,
    ) -> Result<
        Option<(
            FightEntityInfo,
            AssistBossInfo,
            HeroExAttribute,
            HeroSpAttribute,
        )>,
    > {
        let Some(plan) = plan else {
            return Ok(None);
        };
        let tables = catalog.game_data();
        let support = tables
            .tower_compose_support
            .get(plan.support_id)
            .expect("validated Tower Compose support");

        let hero = roster.compose_support.as_ref().ok_or_else(|| {
            anyhow::anyhow!("battle roster has no selected Tower Compose support")
        })?;
        if let Some(assist) = &plan.assist {
            ensure!(assist.hero_id == support.hero_id);
            ensure!(hero.uid == assist.hero_uid);
            ensure!(hero.hero_id == assist.hero_id);
            ensure!(hero.level == assist.level);
            ensure!(hero.skin == assist.skin);
            ensure!(hero.ex_skill_level == assist.ex_skill_level);
        }
        ensure!(hero.hero_id == support.hero_id);
        ensure!(hero.ex_skill_level == support.lv);

        let stats = Stats::configured(tables, &StatInputs::from_build_input(hero, None));
        let attr = stats.base();
        let entity = FightEntityInfo {
            uid: Some(SUPPORT_UID),
            model_id: Some(support.id),
            skin: Some(hero.skin),
            position: Some(0),
            entity_type: Some(5),
            user_id: Some(roster.user_id),
            ex_point: Some(0),
            level: Some(hero.level),
            current_hp: attr.hp,
            attr: Some(attr),
            passive_skill: ids(&support.passive_skills),
            ex_skill: Some(0),
            shield_value: Some(0),
            expoint_max_add: Some(0),
            buff_harm_statistic: Some(0),
            equip_uid: Some(0),
            trial_equip: Some(EquipRecord::default()),
            ex_skill_level: Some(hero.ex_skill_level),
            power_infos: vec![PowerInfo {
                power_id: Some(PowerType::AssistBoss.id()),
                num: Some(support.res_init_val),
                max: Some(support.res_max_val),
            }],
            base_attr: Some(attr),
            ex_skill_point_change: Some(0),
            team_type: Some(1),
            enhance_info_box: Some(EnhanceInfoBox {
                uid: Some(SUPPORT_UID),
                ..Default::default()
            }),
            trial_id: Some(0),
            career: tables
                .character
                .get(support.hero_id)
                .map(|hero| hero.career),
            status: Some(0),
            guard: Some(-1),
            sub_cd: Some(0),
            ex_point_type: Some(0),
            destiny_stone: Some(0),
            destiny_rank: Some(0),
            custom_unit_id: Some(support.active_type),
            ..Default::default()
        };
        let info = AssistBossInfo {
            skills: active_skills(&support.active_skills),
            curr_cd: Some(0),
            cd_cfg: Some(support.cold_time),
            form_id: Some(0),
            round_use_limit: Some(1),
            exceed_use_free: Some(0),
            params: None,
            r#type: Some(support.active_type),
        };
        Ok(Some((entity, info, stats.ex(), stats.sp())))
    }
}

fn attacker_setup(
    catalog: crate::catalog::BattleCatalog,
    episode_id: i32,
    battle_id: i32,
    is_balance: bool,
    fight_group: &sonettobuf::FightGroup,
    params: Option<&str>,
) -> Result<AttackerSetup> {
    let battle = catalog
        .game_data()
        .battle
        .get(battle_id)
        .ok_or_else(|| anyhow::anyhow!("unknown battle {battle_id}"))?;
    let balance = is_balance
        .then(|| {
            BattleBalance::parse(&battle.balance)
                .ok_or_else(|| anyhow::anyhow!("invalid balance config for battle {battle_id}"))
        })
        .transpose()?;
    let aid_ids = configured_aid_ids(&battle.aid)?;
    let trial_heroes = configured_trial_heroes(&battle.trial_heros)?;
    let selected_trials = selected_trial_heroes(
        fight_group,
        &trial_heroes,
        battle.trial_limit,
        battle.role_num,
    )?;
    let main_trial_count = selected_trials
        .iter()
        .filter(|(_, position)| *position > 0)
        .count();
    validate_composition(
        fight_group,
        battle
            .role_num
            .saturating_sub(i32::try_from(selected_trials.len())?),
        battle
            .player_max
            .saturating_sub(i32::try_from(main_trial_count)?),
        aid_ids.len(),
    )
    .with_context(|| format!("invalid composition for battle {battle_id}"))?;
    let use_configured_aids = !aid_ids.is_empty()
        && fight_group
            .hero_list
            .iter()
            .chain(&fight_group.sub_hero_list)
            .all(|uid| *uid == 0);

    Ok(AttackerSetup {
        role_num: battle.role_num,
        balance,
        aid_ids,
        selected_trials,
        use_configured_aids,
        compose_support: compose_support_plan(catalog, episode_id, fight_group, params)?,
    })
}

fn compose_support_plan(
    catalog: crate::catalog::BattleCatalog,
    episode_id: i32,
    fight_group: &sonettobuf::FightGroup,
    params: Option<&str>,
) -> Result<Option<ComposeSupportPlan>> {
    let tables = catalog.game_data();
    if !tables
        .tower_compose_episode
        .iter()
        .any(|row| row.episode_id == episode_id)
    {
        return Ok(None);
    }
    let params: ComposeParams = serde_json::from_str(
        params.ok_or_else(|| anyhow::anyhow!("missing Tower Compose battle params"))?,
    )?;
    ensure!(tables.tower_compose_episode.iter().any(|row| {
        row.episode_id == episode_id
            && row.theme_id == params.theme_id
            && row.layer_id == params.layer_id
            && row.plane == params.plane_id
    }));
    if params.support_id == 0 {
        return Ok(None);
    }

    let group: ComposeGroupParams = serde_json::from_str(
        fight_group
            .params
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing Compose group params"))?,
    )?;
    ensure!(selected_support(&group.buff_params_str, params.plane_id) == Some(params.support_id));
    let support = tables
        .tower_compose_support
        .get(params.support_id)
        .ok_or_else(|| anyhow::anyhow!("unknown Tower Compose support {}", params.support_id))?;
    ensure!(support.theme_id == params.theme_id);

    let assist = if params.support_assist_uid == 0 {
        None
    } else {
        let assist = group
            .assist_data_map
            .get(params.theme_id.saturating_sub(1) as usize)
            .and_then(Option::as_ref)
            .and_then(|planes| planes.get(&params.plane_id.to_string()))
            .ok_or_else(|| anyhow::anyhow!("missing selected Tower Compose assist"))?;
        ensure!(assist.hero_uid == params.support_assist_uid);
        ensure!(assist.hero_id == support.hero_id);
        ensure!(assist.assist_type == params.support_assist_type);
        Some(assist.clone())
    };

    Ok(Some(ComposeSupportPlan {
        support_id: params.support_id,
        hero_id: support.hero_id,
        assist,
    }))
}

fn validate_composition(
    fight_group: &sonettobuf::FightGroup,
    role_num: i32,
    player_max: i32,
    aid_count: usize,
) -> Result<()> {
    let role_num = role_num.max(0) as usize;
    let player_max = player_max.max(0) as usize;
    ensure!(fight_group.hero_list.len() <= role_num);
    ensure!(fight_group.sub_hero_list.len() <= role_num);
    ensure!(
        fight_group
            .hero_list
            .iter()
            .filter(|uid| **uid != 0)
            .count()
            <= player_max
    );
    ensure!(
        fight_group
            .sub_hero_list
            .iter()
            .filter(|uid| **uid != 0)
            .count()
            <= role_num.saturating_sub(player_max)
    );

    let selected = fight_group
        .hero_list
        .iter()
        .chain(&fight_group.sub_hero_list)
        .copied()
        .filter(|uid| *uid != 0)
        .collect::<Vec<_>>();
    ensure!(selected.len() <= role_num);
    if selected.is_empty() {
        ensure!(aid_count <= role_num);
        ensure!(aid_count <= player_max);
    }
    ensure!(
        selected
            .iter()
            .all(|uid| { *uid > 0 || (*uid < 0 && uid.unsigned_abs() <= aid_count as u64) }),
        "selected heroes {selected:?} reference configured aids outside 1..={aid_count}"
    );
    ensure!(selected.iter().copied().collect::<HashSet<_>>().len() == selected.len());
    Ok(())
}

fn configured_aid(
    catalog: crate::catalog::BattleCatalog,
    aid_ids: &[i32],
    uid: i64,
    position: i32,
) -> Result<Option<FightEntityInfo>> {
    if uid >= 0 {
        return Ok(None);
    }
    let index = usize::try_from(uid.unsigned_abs() - 1)
        .map_err(|_| anyhow::anyhow!("configured aid uid {uid} is out of range"))?;
    let monster_id = *aid_ids
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("configured aid uid {uid} is not declared by the battle"))?;
    Ok(Some(Defender::build_monster(
        catalog, monster_id, uid, position, 1,
    )?))
}

fn configured_aid_ids(raw: &str) -> Result<Vec<i32>> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split('#')
        .map(|value| {
            value
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid configured aid monster id '{value}'"))
        })
        .collect()
}

fn configured_trial_heroes(raw: &str) -> Result<HashMap<i32, Option<i32>>> {
    raw.split('|')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let mut values = entry.split('#');
            let trial_id = values
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| anyhow::anyhow!("invalid configured trial hero '{entry}'"))?;
            let position = values
                .nth(1)
                .map(str::parse)
                .transpose()
                .map_err(|_| anyhow::anyhow!("invalid configured trial position '{entry}'"))?;
            Ok((trial_id, position))
        })
        .collect()
}

fn selected_trial_heroes(
    fight_group: &sonettobuf::FightGroup,
    configured: &HashMap<i32, Option<i32>>,
    trial_limit: i32,
    role_num: i32,
) -> Result<Vec<(i32, i32)>> {
    ensure!(fight_group.trial_hero_list.len() <= trial_limit.max(0) as usize);
    let mut ids = HashSet::new();
    let mut positions = HashSet::new();
    fight_group
        .trial_hero_list
        .iter()
        .map(|selected| {
            let trial_id = selected.trial_id.unwrap_or_default();
            ensure!(ids.insert(trial_id), "duplicate trial hero {trial_id}");
            let configured_position = configured
                .get(&trial_id)
                .ok_or_else(|| anyhow::anyhow!("trial hero {trial_id} is not allowed"))?;
            let position = selected.pos.or(*configured_position).unwrap_or(-1);
            ensure!(
                position == -1 || (1..=role_num).contains(&position),
                "invalid trial hero position {position}"
            );
            ensure!(
                position == -1 || positions.insert(position),
                "duplicate trial hero position {position}"
            );
            Ok((trial_id, position))
        })
        .collect()
}

fn reserved_uid_offset(
    fight_group: &sonettobuf::FightGroup,
    configured_aid_count: usize,
    has_fixed_support: bool,
) -> Result<usize> {
    let aid_offset = fight_group
        .hero_list
        .iter()
        .chain(&fight_group.sub_hero_list)
        .filter(|uid| **uid < 0)
        .filter_map(|uid| usize::try_from(uid.unsigned_abs()).ok())
        .max()
        .unwrap_or_default()
        .max(configured_aid_count);
    ensure!(
        !has_fixed_support || aid_offset == 0,
        "configured aid uid collides with fixed support uid"
    );
    Ok(aid_offset.max(usize::from(has_fixed_support)))
}

fn selected_support(raw: &str, plane_id: i32) -> Option<i32> {
    let plane = if plane_id == 0 {
        raw
    } else {
        raw.split('|').nth(plane_id.saturating_sub(1) as usize)?
    };
    plane.split('#').next()?.parse().ok()
}

fn ids(raw: &str) -> Vec<i32> {
    raw.split(['#', '|'])
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn active_skills(raw: &str) -> Vec<AssistBossSkillInfo> {
    raw.split('|')
        .filter_map(|entry| {
            let mut values = entry.split('#').filter_map(|value| value.parse().ok());
            Some(AssistBossSkillInfo {
                skill_id: Some(values.next()?),
                need_power: Some(values.next()?),
                power_low: Some(values.next()?),
                power_high: Some(values.next()?),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "attacker/tests.rs"]
mod tests;
