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
    research_id: i32,
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
    assist_data_map: serde_json::Value,
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
    compose_portrait_increments: HashMap<i32, i32>,
    compose_portrait_marker: Option<i32>,
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
        // Assist bosses and compose supports occupy uid -1. Trial heroes have
        // server-assigned uids, so reserve that slot before assigning them.
        let has_fixed_support = setup.compose_support.is_some()
            || fight_group.assist_boss_id.unwrap_or_default() != 0;
        let portrait_increments = setup.compose_portrait_increments;
        let portrait_marker = setup.compose_portrait_marker;
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
            let uid = -i64::try_from(aid_ids.len() + index + 1 + usize::from(has_fixed_support))?;
            let original_level = game.hero_trial.get(trial_id)
                .with_context(|| format!("unknown trial hero {trial_id}"))?.ex_skill_lv;
            let adjusted_level = research_portrait_level(original_level, position, &portrait_increments);
            let (mut entity, stats) = EntityBuilder::configured_trial_with_ex_level(
                catalog, trial_id, uid, position, 1, Some(adjusted_level))?;
            if adjusted_level > original_level {
                if let Some(marker) = portrait_marker { entity.passive_skill.push(marker); }
            }
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
            let mut hero_input = fighter.hero.clone();
            let original_level = hero_input.ex_skill_level;
            hero_input.ex_skill_level = research_portrait_level(original_level, position, &portrait_increments);
            let equip_inputs = fighter.equips.clone();
            let stats = balance
                .map(|balance| balance.stats(game, &hero_input, &equip_inputs))
                .unwrap_or_else(|| Stats::loadout(game, &hero_input, &equip_inputs));
            ex_attributes.push((hero_input.uid, stats.ex()));
            sp_attributes.push((hero_input.uid, stats.sp()));

            let mut builder = EntityBuilder::new(hero_input, position, 1, false)
                .with_catalog(catalog)
                .with_equips(equip_inputs)
                .with_stats(stats);
            if let Some(balance) = balance {
                builder = builder.with_balance(balance, stats);
            }
            let mut entity = builder.build()?;
            if entity.ex_skill_level.unwrap_or_default() > original_level {
                if let Some(marker) = portrait_marker { entity.passive_skill.push(marker); }
            }
            entitys.push(entity);
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

            let mut builder = EntityBuilder::new(hero_input, -1, 1, true)
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
            Some(catalog.cloth_initial_power(fight_group.cloth_id).unwrap_or(15)),
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
            if use_configured_aids { aid_ids.len() } else { 0 },
            has_fixed_support,
        )?.max(if selected_trials.is_empty() { 0 } else {
            aid_ids.len() + selected_trials.len() + usize::from(has_fixed_support)
        });

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

    let (compose_portrait_increments, compose_portrait_marker) =
        compose_portrait_research(catalog, episode_id, params)?;
    Ok(AttackerSetup {
        role_num: battle.role_num,
        balance,
        aid_ids,
        selected_trials,
        use_configured_aids,
        compose_support: compose_support_plan(catalog, episode_id, fight_group, params)?,
        compose_portrait_increments,
        compose_portrait_marker,
    })
}

fn research_portrait_level(original: i32, position: i32, increments: &HashMap<i32, i32>) -> i32 {
    increments.get(&position).map_or(original, |amount| {
        original.saturating_add(*amount).min(5).max(original)
    })
}

fn compose_portrait_research(
    catalog: crate::catalog::BattleCatalog, episode_id: i32, params: Option<&str>,
) -> Result<(HashMap<i32, i32>, Option<i32>)> {
    let db = catalog.game_data();
    if !db.tower_compose_episode.iter().any(|row| row.episode_id == episode_id) {
        return Ok((HashMap::new(), None));
    }
    let params: ComposeParams = serde_json::from_str(
        params.context("missing Tower Compose battle params")?)?;
    if params.research_id == 0 { return Ok((HashMap::new(), None)); }
    let research = db.tower_compose_research.get(params.research_id)
        .filter(|row| row.theme_id == params.theme_id)
        .context("unknown Tower Compose research")?;
    if research.ex_level.trim().is_empty() { return Ok((HashMap::new(), None)); }
    let mut increments = HashMap::new();
    for entry in research.ex_level.split('|') {
        let (position, amount) = entry.split_once('#').context("invalid research portrait entry")?;
        let position = position.trim().parse::<i32>()?;
        let amount = amount.trim().parse::<i32>()?;
        ensure!(position > 0 && amount > 0 && increments.insert(position, amount).is_none(),
            "invalid research portrait adjustment");
    }
    let marker = db.tower_compose_const.get(10).context("missing portrait research marker")?
        .value.trim().parse::<i32>()?;
    ensure!(db.skill.get(marker).is_some(), "unknown portrait research marker");
    Ok((increments, Some(marker)))
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
            && compose_plane_matches(row.plane, params.plane_id)
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
        let assist = compose_assist_data(&group.assist_data_map, params.theme_id, params.plane_id)
            .ok_or_else(|| anyhow::anyhow!("missing selected Tower Compose assist"))?;
        let assist: ComposeAssist = serde_json::from_value(assist.clone())
            .context("invalid selected Tower Compose assist")?;
        ensure!(assist.hero_uid == params.support_assist_uid);
        ensure!(assist.hero_id == support.hero_id);
        ensure!(assist.assist_type == params.support_assist_type);
        Some(assist)
    };

    Ok(Some(ComposeSupportPlan {
        support_id: params.support_id,
        hero_id: support.hero_id,
        assist,
    }))
}

fn compose_plane_matches(configured: i32, selected: i32) -> bool {
    match configured {
        0 => selected == 0,
        1 => selected == 1,
        2 => selected == 1 || selected == 2,
        _ => false,
    }
}

// Lua tables may encode as arrays or objects, and empty, unselected slots
// contain no hero fields. Parse only the slot the request actually selects.
fn compose_assist_data(
    map: &serde_json::Value,
    theme_id: i32,
    plane_id: i32,
) -> Option<&serde_json::Value> {
    let theme = match map {
        serde_json::Value::Array(themes) => themes.get(usize::try_from(theme_id).ok()?.checked_sub(1)?)?,
        serde_json::Value::Object(themes) if theme_id > 0 => themes.get(&theme_id.to_string())?,
        _ => return None,
    };
    match theme {
        serde_json::Value::Array(planes) => planes.get(usize::try_from(plane_id).ok()?.checked_sub(1)?),
        serde_json::Value::Object(planes) if plane_id >= 0 => planes.get(&plane_id.to_string()),
        _ => None,
    }
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
mod tests {
    use super::{
        BattleFighter, BattleRoster, BattleRosterPlan, configured_aid_ids, plan_roster,
        compose_plane_matches, compose_support_plan, reserved_uid_offset, selected_support,
        validate_composition,
    };
    use crate::dungeon::FightOptions;
    use crate::engine::entity::input::{EquipmentBuildInput, HeroBuildInput};

    #[test]
    fn compose_portrait_research_rebuilds_only_configured_positions() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let specs = [(3088, 308801, 2), (3125, 312501, 5), (3126, 312601, 1), (3120, 312001, 0)];
        let fighters = specs.into_iter().enumerate().map(|(index, (hero_id, skin, ex_skill_level))| {
            BattleFighter { hero: HeroBuildInput {
                uid: 20_000_001 + index as i64, user_id: 1, hero_id, skin,
                level: 180, rank: 4, talent: 15, ex_skill_level, ..Default::default()
            }, equips: vec![] }
        }).collect::<Vec<_>>();
        let group = sonettobuf::FightGroup {
            hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
        };
        let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
        let base = r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0,"researchId":0}"#;
        let research = r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0,"researchId":1403}"#;
        let build = |params| super::super::build_fight(catalog, &roster, 90400199, 9001001,
            &group, FightOptions::default(), Some(params)).unwrap();
        let baseline = build(base);
        let changed = build(research);
        let before = &baseline.fight.attacker.as_ref().unwrap().entitys;
        let after = &changed.fight.attacker.as_ref().unwrap().entitys;
        assert_eq!(after.iter().map(|hero| hero.ex_skill_level.unwrap()).collect::<Vec<_>>(), vec![3, 5, 1, 0]);
        assert_ne!(after[0].ex_skill, before[0].ex_skill);
        assert!(after[0].passive_skill.contains(&113330011), "newly upgraded hero needs the marker");
        assert!(!after[1].passive_skill.contains(&113330011), "already-maxed hero gets the separate bonus");
        assert!(!after[2].passive_skill.contains(&113330011));
        assert_eq!(after[2].ex_skill, before[2].ex_skill);
        assert_eq!(roster.fighters[0].hero.ex_skill_level, 2, "research must not change the saved roster");
    }

    #[test]
    fn selects_support_from_the_active_compose_plane() {
        assert_eq!(selected_support("30#40#50", 0), Some(30));
        assert_eq!(selected_support("31#41#51|32#42#52", 2), Some(32));
    }

    #[test]
    fn compose_two_plane_layer_accepts_either_plane() {
        assert!(compose_plane_matches(2, 1));
        assert!(compose_plane_matches(2, 2));
        assert!(!compose_plane_matches(2, 0));
        assert!(!compose_plane_matches(1, 2));
        crate::test_support::init_config();
        let first_plane = r#"{"themeId":1,"layerId":6,"planeId":1,"supportId":0}"#;
        assert!(compose_support_plan(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            90400199, &sonettobuf::FightGroup::default(), Some(first_plane),
        ).unwrap().is_none());
    }

    #[test]
    fn compose_support_allows_empty_client_assist_slots_for_owned_heroes() {
        crate::test_support::init_config();
        let group = sonettobuf::FightGroup {
            params: Some(r#"{"heroList":["0","0","0","0"],"assistDataMap":[{"0":{}}],"buffParamsStr":"306600#0#0"}"#.into()),
            ..Default::default()
        };
        let params = r#"{"supportAssistUid":0,"layerId":1,"supportId":306600,"supportAssistType":0,"themeId":1,"planeId":0}"#;
        let plan = compose_support_plan(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            90400101, &group, Some(params),
        ).unwrap().unwrap();
        assert_eq!(plan.hero_id, 3066);
        assert!(plan.assist.is_none());
    }

    #[test]
    fn compose_support_reads_selected_borrowed_hero_from_lua_plane_arrays() {
        crate::test_support::init_config();
        let group = sonettobuf::FightGroup {
            params: Some(r#"{"assistDataMap":[[{}, {"heroUid":20000001,"heroId":3066,"level":1,"skin":306601,"exSkillLevel":0,"assistType":6}]],"buffParamsStr":"0#0#0|306600#0#0"}"#.into()),
            ..Default::default()
        };
        let params = r#"{"supportAssistUid":20000001,"layerId":6,"supportId":306600,"supportAssistType":6,"themeId":1,"planeId":2}"#;
        let plan = compose_support_plan(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            90400199, &group, Some(params),
        ).unwrap().unwrap();
        assert_eq!(plan.assist.unwrap().hero_uid, 20_000_001);
    }

    #[test]
    fn compose_support_reads_selected_object_slot_and_rejects_invalid_borrowed_heroes() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let params = r#"{"supportAssistUid":20000001,"layerId":1,"supportId":306600,"supportAssistType":6,"themeId":1,"planeId":0}"#;
        let mut group = sonettobuf::FightGroup {
            params: Some(r#"{"assistDataMap":{"1":{"0":{"heroUid":20000001,"heroId":3066,"level":1,"skin":306601,"exSkillLevel":0,"assistType":6},"1":{}}},"buffParamsStr":"306600#0#0"}"#.into()),
            ..Default::default()
        };
        let plan = compose_support_plan(catalog, 90400101, &group, Some(params)).unwrap().unwrap();
        assert_eq!(plan.assist.unwrap().hero_uid, 20_000_001);

        group.params = Some(r#"{"assistDataMap":[{"0":{}}],"buffParamsStr":"306600#0#0"}"#.into());
        let error = compose_support_plan(catalog, 90400101, &group, Some(params)).err().unwrap();
        assert!(error.to_string().contains("invalid selected Tower Compose assist"));

        group.params = Some(r#"{"assistDataMap":[{"0":{"heroUid":20000002,"heroId":3066,"level":1,"skin":306601,"exSkillLevel":0,"assistType":6}}],"buffParamsStr":"306600#0#0"}"#.into());
        assert!(compose_support_plan(catalog, 90400101, &group, Some(params)).is_err());
    }

    #[test]
    fn explicit_roster_plan_matches_the_legacy_entry_point() {
        crate::test_support::init_config();
        let group = sonettobuf::FightGroup::default();
        let legacy = plan_roster(10101, 10101, false, &group, None).unwrap();
        let explicit = BattleRosterPlan::configured(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            10101,
            10101,
            false,
            &group,
            None,
        )
        .unwrap();

        assert_eq!(explicit.hero_uids(), legacy.hero_uids());
        assert_eq!(explicit.compose_support(), legacy.compose_support());
    }

    #[test]
    fn validates_configured_team_slots_and_unique_hero_uids() {
        let valid = sonettobuf::FightGroup {
            hero_list: vec![10, 11, 12],
            sub_hero_list: vec![13],
            ..Default::default()
        };
        assert!(validate_composition(&valid, 4, 3, 0).is_ok());

        let placeholder_slots = sonettobuf::FightGroup {
            hero_list: vec![-1, -2],
            sub_hero_list: vec![0, 0],
            ..Default::default()
        };
        assert!(validate_composition(&placeholder_slots, 3, 2, 2).is_ok());

        let excessive_placeholders = sonettobuf::FightGroup {
            sub_hero_list: vec![0, 0, 0, 0],
            ..Default::default()
        };
        assert!(validate_composition(&excessive_placeholders, 3, 2, 0).is_err());

        let duplicate = sonettobuf::FightGroup {
            sub_hero_list: vec![12],
            ..valid.clone()
        };
        assert!(validate_composition(&duplicate, 4, 3, 0).is_err());

        let oversized_main = sonettobuf::FightGroup {
            hero_list: vec![10, 11, 12, 13],
            ..Default::default()
        };
        assert!(validate_composition(&oversized_main, 4, 3, 0).is_err());

        let oversized_reserve = sonettobuf::FightGroup {
            hero_list: vec![10, 11],
            sub_hero_list: vec![12, 13],
            ..Default::default()
        };
        assert!(validate_composition(&oversized_reserve, 3, 2, 0).is_err());

        let configured_aids = sonettobuf::FightGroup {
            hero_list: vec![-1, -2],
            ..Default::default()
        };
        assert!(validate_composition(&configured_aids, 3, 3, 2).is_ok());
        assert!(validate_composition(&configured_aids, 3, 3, 1).is_err());
        assert_eq!(
            configured_aid_ids("100102#100101").unwrap(),
            vec![100102, 100101]
        );
        assert!(configured_aid_ids("100102#bad#100101").is_err());
        assert!(reserved_uid_offset(&configured_aids, 0, true).is_err());
        assert_eq!(
            reserved_uid_offset(&sonettobuf::FightGroup::default(), 0, true).unwrap(),
            1
        );
        assert!(reserved_uid_offset(&sonettobuf::FightGroup::default(), 2, true).is_err());
    }

    #[test]
    fn captured_tutorial_placeholders_build_the_configured_aid_team() {
        crate::test_support::init_config();
        let fight_group = sonettobuf::FightGroup {
            hero_list: vec![-1, -2],
            sub_hero_list: vec![0, 0],
            ..Default::default()
        };

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &BattleRoster {
                user_id: 1,
                ..Default::default()
            },
            10103,
            11021,
            &fight_group,
            FightOptions::default(),
            None,
        )
        .unwrap();
        let attacker = built.fight.attacker.unwrap();

        assert_eq!(
            attacker
                .entitys
                .iter()
                .map(|entity| (entity.uid.unwrap(), entity.model_id.unwrap()))
                .collect::<Vec<_>>(),
            vec![(-1, 110205), (-2, 110204)]
        );
        assert!(attacker.sub_entitys.is_empty());
        assert!(
            attacker
                .entitys
                .iter()
                .all(|entity| entity.passive_skill.contains(&72_010))
        );
    }

    #[test]
    fn configured_trial_hero_uses_its_loadout_and_reserved_position() {
        crate::test_support::init_config();
        let fight_group = sonettobuf::FightGroup {
            trial_hero_list: vec![sonettobuf::TrialHero {
                trial_id: Some(4301003),
                pos: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        };

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &BattleRoster {
                user_id: 1,
                ..Default::default()
            },
            1102,
            201101,
            &fight_group,
            FightOptions::default(),
            None,
        )
        .unwrap();
        let attacker = built.fight.attacker.unwrap();
        let trial = &attacker.entitys[0];

        assert_eq!(
            (
                trial.uid,
                trial.model_id,
                trial.position,
                trial.trial_id,
                trial.trial_equip.as_ref().and_then(|equip| equip.equip_id),
            ),
            (Some(-1), Some(3126), Some(1), Some(4301003), Some(1553))
        );
        assert_eq!(
            trial.attr.as_ref().map(|attr| {
                (
                    attr.hp,
                    attr.attack,
                    attr.defense,
                    attr.mdefense,
                    attr.technic,
                )
            }),
            Some((Some(6015), Some(735), Some(291), Some(296), Some(370)))
        );
        assert_eq!(
            trial.passive_skill,
            vec![31260141, 31260191, 435311, 530000151]
        );
        assert_eq!(trial.ex_point_max, Some(5));
        assert!(!trial.skill_group1.is_empty());
        assert!(!trial.passive_skill.is_empty());
    }

    #[test]
    fn linked_trial_psychube_contributes_stats_and_passives() {
        crate::test_support::init_config();
        let (trial, stats) =
            crate::engine::entity::builder::EntityBuilder::trial(116385001, -1, 1, 1).unwrap();

        assert_eq!(
            (stats.hp, stats.atk, stats.def, stats.mdef),
            (14610, 2469, 1005, 1005)
        );
        assert!(trial.passive_skill.contains(&437115));
        assert!(trial.passive_skill.contains(&437215));
    }

    #[test]
    fn configured_aids_build_as_allied_monsters_and_reserve_their_uids() {
        crate::test_support::init_config();
        let fight_group = sonettobuf::FightGroup::default();
        let roster = BattleRoster {
            user_id: 1,
            ..Default::default()
        };

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &roster,
            10002,
            1002,
            &fight_group,
            FightOptions::default(),
            None,
        )
        .unwrap();
        let mut runtime = crate::engine::runtime::BattleRuntime::new(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            built.fight.clone(),
        );
        let round = runtime.start_round().unwrap();
        let cards = runtime.card_info_push();
        let deal = round
            .team_a_cards1
            .iter()
            .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
            .collect::<Vec<_>>();
        // Captured episode 10002: seven cards as dealt, composed to five.
        assert_eq!(
            deal,
            vec![
                (-1, 30250121),
                (-1, 30250121),
                (-1, 30250121),
                (-2, 30230111),
                (-2, 30230111),
                (-2, 30230121),
                (-1, 30250121),
            ]
        );
        assert_eq!(
            cards
                .deal_card_group
                .iter()
                .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
                .collect::<Vec<_>>(),
            deal
        );
        let opening = cards
            .card_group
            .into_iter()
            .map(|card| (card.uid.unwrap(), card.skill_id.unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(
            opening,
            vec![
                (-1, 30250122),
                (-1, 30250121),
                (-2, 30230112),
                (-2, 30230121),
                (-1, 30250121),
            ]
        );
        assert_eq!(opening[4], opening[1]);
        let attacker = built.fight.attacker.unwrap();
        let defender = built.fight.defender.unwrap();

        assert_eq!(
            attacker
                .entitys
                .iter()
                .map(|entity| (entity.uid, entity.model_id, entity.team_type))
                .collect::<Vec<_>>(),
            vec![
                (Some(-1), Some(100102), Some(1)),
                (Some(-2), Some(100101), Some(1)),
            ]
        );
        assert_eq!(
            defender
                .entitys
                .iter()
                .map(|entity| (entity.uid, entity.model_id))
                .collect::<Vec<_>>(),
            vec![(Some(-3), Some(100105)), (Some(-4), Some(100106))]
        );

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &roster,
            10003,
            1003,
            &fight_group,
            FightOptions::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            built
                .fight
                .attacker
                .unwrap()
                .entitys
                .iter()
                .map(|entity| (entity.uid, entity.model_id))
                .collect::<Vec<_>>(),
            vec![(Some(-1), Some(100109))]
        );
        assert_eq!(built.fight.defender.unwrap().entitys[0].uid, Some(-2));
    }

    #[test]
    fn unequipped_character_has_no_equip_record() {
        crate::test_support::init_config();
        let roster = BattleRoster {
            user_id: 1,
            fighters: vec![BattleFighter {
                hero: HeroBuildInput {
                    uid: 20_000_001,
                    user_id: 1,
                    hero_id: 3028,
                    skin: 302801,
                    level: 1,
                    ..Default::default()
                },
                equips: vec![],
            }],
            ..Default::default()
        };
        let group = sonettobuf::FightGroup {
            hero_list: vec![20_000_001],
            ..Default::default()
        };

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &roster,
            10101,
            10101,
            &group,
            FightOptions::default(),
            None,
        )
        .unwrap();
        let entity = &built.fight.attacker.unwrap().entitys[0];

        assert_eq!(entity.equip_uid, Some(0));
        assert!(entity.equips.is_empty());
    }

    #[test]
    fn linked_psychube_is_loaded_with_its_refined_passive() {
        crate::test_support::init_config();
        assert_eq!(
            config::configs::get().linked_psychube_id(3149, 1571),
            Some(1572)
        );
        assert_eq!(config::configs::get().linked_psychube_id(3028, 1571), None);
        let hero_uid = 20_000_001;
        let primary_uid = 30_000_001;
        let linked_uid = 30_000_002;
        let roster = BattleRoster {
            user_id: 1,
            fighters: vec![BattleFighter {
                hero: HeroBuildInput {
                    uid: hero_uid,
                    user_id: 1,
                    hero_id: 3149,
                    skin: 314901,
                    level: 1,
                    ..Default::default()
                },
                equips: vec![
                    EquipmentBuildInput {
                        uid: primary_uid,
                        equip_id: 1571,
                        level: 1,
                        break_level: 1,
                        refine_level: 1,
                    },
                    EquipmentBuildInput {
                        uid: linked_uid,
                        equip_id: 1572,
                        level: 1,
                        break_level: 1,
                        refine_level: 5,
                    },
                ],
            }],
            ..Default::default()
        };
        let group = sonettobuf::FightGroup {
            hero_list: vec![hero_uid],
            ..Default::default()
        };

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &roster,
            10101,
            10101,
            &group,
            FightOptions::default(),
            None,
        )
        .unwrap();
        let entity = &built.fight.attacker.unwrap().entitys[0];

        assert_eq!(entity.equip_uid, Some(primary_uid));
        assert_eq!(
            entity
                .equips
                .iter()
                .map(|equip| (equip.equip_uid, equip.equip_id, equip.refine_lv))
                .collect::<Vec<_>>(),
            vec![
                (Some(primary_uid), Some(1571), Some(1)),
                (Some(linked_uid), Some(1572), Some(5)),
            ]
        );
        assert!(entity.passive_skill.contains(&437111));
        assert!(entity.passive_skill.contains(&437215));
    }

    #[test]
    fn configured_balance_floors_only_the_battle_loadout() {
        crate::test_support::init_config();
        let hero_uid = 20_000_001;
        let equip_uid = 30_000_001;
        let hero = HeroBuildInput {
            uid: hero_uid,
            user_id: 1,
            hero_id: 3127,
            skin: 312701,
            level: 121,
            rank: 4,
            talent: 10,
            ..Default::default()
        };
        let roster = BattleRoster {
            user_id: 1,
            fighters: vec![BattleFighter {
                hero: hero.clone(),
                equips: vec![EquipmentBuildInput {
                    uid: equip_uid,
                    equip_id: 1502,
                    level: 50,
                    break_level: 1,
                    refine_level: 1,
                }],
            }],
            ..Default::default()
        };
        let group = sonettobuf::FightGroup {
            hero_list: vec![hero_uid],
            equips: vec![sonettobuf::FightEquip {
                hero_uid: Some(hero_uid),
                equip_uid: vec![equip_uid],
                ..Default::default()
            }],
            trial_hero_list: vec![sonettobuf::TrialHero {
                trial_id: Some(116385001),
                pos: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        };

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &roster,
            38510113,
            116385108,
            &group,
            FightOptions {
                is_balance: true,
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let entity = built
            .fight
            .attacker
            .unwrap()
            .entitys
            .into_iter()
            .find(|entity| entity.uid == Some(hero_uid))
            .unwrap();

        assert_eq!(entity.level, Some(140));
        assert_eq!(
            entity.attr.map(|attr| {
                (
                    attr.hp,
                    attr.attack,
                    attr.defense,
                    attr.mdefense,
                    attr.technic,
                )
            }),
            Some((Some(9628), Some(1737), Some(820), Some(858), Some(504)))
        );
        assert_eq!(entity.equips[0].equip_lv, Some(60));
        assert_eq!(roster.fighters[0].hero, hero);
    }

    #[test]
    fn requested_balance_requires_valid_battle_config() {
        crate::test_support::init_config();

        let error = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &BattleRoster {
                user_id: 1,
                ..Default::default()
            },
            10101,
            10101,
            &sonettobuf::FightGroup::default(),
            FightOptions {
                is_balance: true,
                ..Default::default()
            },
            None,
        )
        .err()
        .unwrap();

        assert!(error.to_string().contains("invalid balance config"));
    }

    #[test]
    fn compose_support_is_built_before_defender_uids() {
        crate::test_support::init_config();
        let hero_uid = 20_000_001;
        let support = config::configs::get()
            .tower_compose_support
            .get(306600)
            .unwrap();
        let hero = HeroBuildInput {
            uid: hero_uid,
            user_id: 2,
            hero_id: 3066,
            skin: 306601,
            level: 1,
            ex_skill_level: support.lv,
            ..Default::default()
        };
        let group = sonettobuf::FightGroup {
            params: Some(format!(
                r#"{{"heroList":["0","0","0","0"],"assistDataMap":[{{"0":{{"level":{},"skin":{},"exSkillLevel":{},"heroUid":{},"heroId":3066,"assistType":6}}}}],"buffParamsStr":"306600#0#0"}}"#,
                hero.level, hero.skin, hero.ex_skill_level, hero_uid
            )),
            ..Default::default()
        };
        let params = format!(
            r#"{{"supportAssistUid":{},"layerId":1,"supportId":306600,"supportAssistType":6,"themeId":1,"planeId":0}}"#,
            hero_uid
        );

        let built = super::super::build_fight(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            &BattleRoster {
                user_id: 1,
                compose_support: Some(hero),
                ..Default::default()
            },
            90400101,
            9001101,
            &group,
            FightOptions::default(),
            Some(params.as_str()),
        )
        .unwrap();
        let attacker = built.fight.attacker.unwrap();
        assert_eq!(attacker.assist_boss.unwrap().model_id, Some(306600));
        assert_eq!(built.fight.defender.unwrap().entitys[0].uid, Some(-2),);
    }
}
