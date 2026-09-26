use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use battle::engine::entity::{
    input::{EquipmentBuildInput, HeroBuildInput},
    stats::{BattleBalance, Stats},
};
use sonettobuf::{
    Fight, FightEntityInfo, HeroExAttribute, HeroInfo, HeroInfoListReply, HeroSpAttribute,
    HeroUpdatePush,
};

use crate::normalize_live_json;

type PreviewAttributes = (Vec<(i64, HeroExAttribute)>, Vec<(i64, HeroSpAttribute)>);

pub fn preview_attributes(fight: &Fight, battle_path: &Path) -> anyhow::Result<PreviewAttributes> {
    let metadata = battle_build_metadata(battle_path)?;
    let request = battle_request_metadata(battle_path)?;
    let battle_balance = request_battle_balance(fight, request.is_balance)?;
    let mut ex_attributes = Vec::new();
    let mut sp_attributes = Vec::new();

    for entity in fight
        .attacker
        .iter()
        .flat_map(|team| team.entitys.iter().chain(team.sub_entitys.iter()))
    {
        let uid = entity
            .uid
            .ok_or_else(|| anyhow::anyhow!("attacker is missing uid"))?;
        let model_id = entity
            .model_id
            .filter(|model_id| *model_id > 0)
            .ok_or_else(|| anyhow::anyhow!("attacker {uid} is missing model id"))?;

        if let Some((trial, stats)) = configured_trial(entity)? {
            ex_attributes.push((uid, stats.ex()));
            sp_attributes.push((uid, stats.sp()));
            if battle::engine::diagnostics::enabled(battle::engine::diagnostics::TraceArea::Damage)
            {
                eprintln!(
                    "attribute preview uid={uid} hero={} source=configured-trial",
                    trial.model_id.unwrap_or_default(),
                );
            }
            continue;
        }

        let hero = metadata.get(&uid).ok_or_else(|| {
            anyhow::anyhow!("attribute preview missing build metadata uid={uid} hero={model_id}")
        })?;
        let build = preview_build_input(entity, hero)?;
        let equips =
            validated_equipment_loadout(entity, hero, request.selected_equips.get(&uid).copied())
                .map_err(|()| {
                anyhow::anyhow!(
                    "attribute preview has invalid equipment metadata uid={uid} hero={model_id}",
                )
            })?;
        let stats = battle_balance.map_or_else(
            || Stats::build_for_loadout(&build, &equips),
            |balance| balance.stats_for(&build, &equips),
        );

        if battle::engine::diagnostics::enabled(battle::engine::diagnostics::TraceArea::Damage) {
            eprintln!(
                "attribute preview uid={uid} hero={model_id} source=validated-build build={build:?} stats={stats:?}",
            );
        }
        ex_attributes.push((uid, stats.ex()));
        sp_attributes.push((uid, stats.sp()));
    }

    Ok((ex_attributes, sp_attributes))
}

#[derive(Debug, Default)]
struct BattleRequestMetadata {
    is_balance: bool,
    selected_equips: HashMap<i64, i64>,
}

fn battle_request_metadata(battle_path: &Path) -> anyhow::Result<BattleRequestMetadata> {
    let Some(parent) = battle_path.parent() else {
        return Ok(BattleRequestMetadata::default());
    };
    let request_path = parent.join("StartDungeonRequest.json");
    if !request_path.exists() {
        return Ok(BattleRequestMetadata::default());
    }
    let request: serde_json::Value = serde_json::from_str(&fs::read_to_string(request_path)?)?;
    let request = request
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("invalid dungeon request"))?;
    let is_balance = request
        .get("isBalance")
        .or_else(|| request.get("is_balance"))
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("invalid isBalance value"))
        })
        .transpose()?
        .unwrap_or_default();
    let selections = match request
        .get("fightGroup")
        .or_else(|| request.get("fight_group"))
    {
        Some(group) => match group
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("invalid fight group"))?
            .get("equips")
        {
            Some(equips) => equips
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid equipment selections"))?
                .as_slice(),
            None => &[],
        },
        None => &[],
    };
    let mut selected_equips = HashMap::new();
    let mut selected_heroes = std::collections::HashSet::new();
    for selection in selections {
        let selection = selection
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("invalid equipment selection"))?;
        let hero_uid = json_i64(
            selection
                .get("heroUid")
                .or_else(|| selection.get("hero_uid"))
                .ok_or_else(|| anyhow::anyhow!("equipment selection is missing hero uid"))?,
        )?;
        if hero_uid < 0 {
            anyhow::bail!("equipment selection has invalid hero uid {hero_uid}");
        }
        let equip_uids = selection
            .get("equipUid")
            .or_else(|| selection.get("equip_uid"))
            .map(|uids| {
                uids.as_array()
                    .ok_or_else(|| anyhow::anyhow!("invalid equipment uids for hero {hero_uid}"))
            })
            .transpose()?
            .into_iter()
            .flatten()
            .map(json_i64)
            .collect::<anyhow::Result<Vec<_>>>()?;
        if equip_uids.iter().any(|uid| *uid < 0) {
            anyhow::bail!("negative equipment uid selected for hero {hero_uid}");
        }
        if hero_uid == 0 {
            continue;
        }
        if !selected_heroes.insert(hero_uid) {
            anyhow::bail!("duplicate equipment selection for hero {hero_uid}");
        }
        let mut equip_uids = equip_uids.into_iter().filter(|uid| *uid != 0);
        let Some(equip_uid) = equip_uids.next() else {
            continue;
        };
        if equip_uids.any(|uid| uid != equip_uid) {
            anyhow::bail!("multiple primary equipment selections for hero {hero_uid}");
        }
        selected_equips.insert(hero_uid, equip_uid);
    }
    Ok(BattleRequestMetadata {
        is_balance,
        selected_equips,
    })
}

fn json_i64(value: &serde_json::Value) -> anyhow::Result<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        .ok_or_else(|| anyhow::anyhow!("invalid integer value"))
}

fn request_battle_balance(
    fight: &Fight,
    is_balance: bool,
) -> anyhow::Result<Option<BattleBalance>> {
    if !is_balance {
        return Ok(None);
    }
    let battle_id = fight
        .battle_id
        .filter(|battle_id| *battle_id > 0)
        .ok_or_else(|| anyhow::anyhow!("balanced request missing battle id"))?;
    let battle = config::configs::get()
        .battle
        .get(battle_id)
        .ok_or_else(|| anyhow::anyhow!("unknown battle {battle_id}"))?;
    BattleBalance::parse(&battle.balance)
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("invalid balance config for battle {battle_id}"))
}

fn configured_trial(
    entity: &FightEntityInfo,
) -> anyhow::Result<Option<(FightEntityInfo, battle::engine::entity::stats::Stats)>> {
    let Some(trial_id) = entity.trial_id.filter(|trial_id| *trial_id > 0) else {
        return Ok(None);
    };
    let uid = entity
        .uid
        .ok_or_else(|| anyhow::anyhow!("trial {trial_id} is missing attacker uid"))?;
    let (trial, stats) = battle::engine::entity::builder::EntityBuilder::trial(
        trial_id,
        uid,
        entity.position.unwrap_or_default(),
        entity.team_type.unwrap_or_default(),
    )
    .map_err(|error| anyhow::anyhow!("invalid trial {trial_id}: {error}"))?;
    if trial.model_id != entity.model_id {
        anyhow::bail!(
            "trial {trial_id} hero mismatch: configured={} fight={}",
            trial.model_id.unwrap_or_default(),
            entity.model_id.unwrap_or_default(),
        );
    }
    Ok(Some((trial, stats)))
}

fn validated_equipment_loadout(
    entity: &FightEntityInfo,
    hero: &HeroInfo,
    requested_equip_uid: Option<i64>,
) -> Result<Vec<EquipmentBuildInput>, ()> {
    if requested_equip_uid.is_some_and(|uid| uid < 0)
        || hero.default_equip_uid.is_some_and(|uid| uid < 0)
        || entity.equip_uid.is_some_and(|uid| uid < 0)
    {
        return Err(());
    }
    let expected_equip_uid =
        requested_equip_uid.or_else(|| hero.default_equip_uid.filter(|uid| *uid > 0));
    if entity.equips.is_empty() {
        return match (expected_equip_uid, entity.equip_uid.filter(|uid| *uid > 0)) {
            (None, None) => Ok(Vec::new()),
            _ => Err(()),
        };
    }
    if entity.model_id.filter(|model_id| *model_id > 0) != Some(hero.hero_id) {
        return Err(());
    }
    let selected_equip_uid = entity.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
    if Some(selected_equip_uid) != expected_equip_uid {
        return Err(());
    }
    let primary = entity.equips.first().ok_or(())?;
    if primary.equip_uid != Some(selected_equip_uid) || entity.equips.len() > 2 {
        return Err(());
    }

    let game = config::configs::get();
    let mut equip_uids = HashSet::with_capacity(entity.equips.len());
    for equip in &entity.equips {
        let uid = equip.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
        if !equip_uids.insert(uid) {
            return Err(());
        }
    }
    let primary_equip_id = primary.equip_id.filter(|id| *id > 0).ok_or(())?;
    let primary_equipment = game.equip.get(primary_equip_id).ok_or(())?;
    let primary_level = primary.equip_lv.filter(|level| *level > 0).ok_or(())?;
    game.equip_strengthen_cost(primary_equipment.rare, primary_level)
        .ok_or(())?;
    let primary = EquipmentBuildInput {
        uid: selected_equip_uid,
        equip_id: primary_equip_id,
        level: primary_level,
        break_level: 0,
        refine_level: primary.refine_lv.unwrap_or_default(),
    };

    let linked_equip_id = game.linked_psychube_id(hero.hero_id, primary_equip_id);
    let supplemental = entity
        .equips
        .get(1)
        .map(|equip| {
            let uid = equip.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
            let equip_id = equip.equip_id.filter(|id| *id > 0).ok_or(())?;
            if Some(equip_id) != linked_equip_id {
                return Err(());
            }
            let equipment = game.equip.get(equip_id).ok_or(())?;
            let level = equip.equip_lv.filter(|level| *level > 0).ok_or(())?;
            game.equip_strengthen_cost(equipment.rare, level)
                .ok_or(())?;
            Ok(EquipmentBuildInput {
                uid,
                equip_id,
                level,
                break_level: 0,
                refine_level: equip.refine_lv.unwrap_or_default(),
            })
        })
        .transpose()?;
    Ok(std::iter::once(primary).chain(supplemental).collect())
}

fn battle_build_metadata(path: &Path) -> anyhow::Result<HashMap<i64, HeroInfo>> {
    let Some(parent) = path.parent() else {
        return Ok(HashMap::new());
    };
    let mut roster_files = Vec::new();
    let mut update_files = Vec::new();
    for path in fs::read_dir(parent)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
    {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("HeroInfoListReply") && name.ends_with(".json") {
            roster_files.push(path);
        } else if name.starts_with("HeroUpdatePush") && name.ends_with(".json") {
            update_files.push(path);
        }
    }
    roster_files.sort();
    update_files.sort();

    let mut heroes = HashMap::new();
    for file in roster_files {
        let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(file)?)?;
        normalize_live_json(&mut value);
        let roster: HeroInfoListReply = serde_json::from_value(value)?;
        for hero in roster.heros {
            heroes.insert(hero.uid, hero);
        }
    }
    for file in update_files {
        let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(file)?)?;
        normalize_live_json(&mut value);
        let update: HeroUpdatePush = serde_json::from_value(value)?;
        for hero in update.hero_updates {
            heroes.insert(hero.uid, hero);
        }
    }
    Ok(heroes)
}

fn preview_build_input(
    entity: &FightEntityInfo,
    hero: &HeroInfo,
) -> anyhow::Result<HeroBuildInput> {
    let model_id = entity
        .model_id
        .filter(|model_id| *model_id > 0)
        .ok_or_else(|| anyhow::anyhow!("attacker is missing model id"))?;
    if hero.hero_id != model_id {
        anyhow::bail!(
            "attribute preview build mismatch uid={} fight hero={} metadata hero={}",
            entity.uid.unwrap_or_default(),
            model_id,
            hero.hero_id,
        );
    }
    let level = entity
        .level
        .or(hero.level)
        .filter(|level| *level > 0)
        .ok_or_else(|| anyhow::anyhow!("attribute preview missing hero level for {model_id}"))?;
    let talent = hero
        .talent
        .filter(|talent| *talent >= 0)
        .ok_or_else(|| anyhow::anyhow!("attribute preview missing talent for {model_id}"))?;
    let selected_template = hero.use_talent_template_id.unwrap_or_default();
    let template = if selected_template == 0 {
        None
    } else {
        Some(
            hero.talent_templates
                .iter()
                .find(|template| template.id == Some(selected_template))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "attribute preview missing selected talent template {selected_template} for {model_id}"
                    )
                })?,
        )
    };
    let cubes = template
        .filter(|template| !template.talent_cube_infos.is_empty())
        .map(|template| template.talent_cube_infos.as_slice())
        .unwrap_or(&hero.talent_cube_infos);
    let talent_placements = cubes
        .iter()
        .map(|cube| {
            cube.cube_id
                .filter(|cube_id| *cube_id > 0)
                .ok_or_else(|| anyhow::anyhow!("invalid talent placement for {model_id}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let rank = hero
        .rank
        .filter(|rank| *rank >= 0)
        .ok_or_else(|| anyhow::anyhow!("attribute preview missing rank for {model_id}"))?;
    Ok(HeroBuildInput {
        uid: entity.uid.unwrap_or_default(),
        user_id: hero.user_id,
        hero_id: model_id,
        skin: entity.skin.unwrap_or_default(),
        level,
        rank,
        ex_skill_level: entity.ex_skill_level.unwrap_or_default(),
        talent,
        talent_style: template
            .and_then(|template| template.style)
            .unwrap_or_default(),
        talent_placements,
        destiny_rank: entity
            .destiny_rank
            .or(hero.destiny_rank)
            .unwrap_or_default(),
        destiny_stone: entity.destiny_stone.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{EquipRecord, FightTeam, TalentCubeInfo};

    fn test_directory(label: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "enigma-preview-attributes-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        directory
    }

    fn fight(uid: i64) -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(uid),
                    model_id: Some(3149),
                    level: Some(60),
                    equip_uid: Some(100),
                    equips: vec![
                        EquipRecord {
                            equip_uid: Some(100),
                            equip_id: Some(1571),
                            equip_lv: Some(60),
                            ..Default::default()
                        },
                        EquipRecord {
                            equip_uid: Some(200),
                            equip_id: Some(1572),
                            equip_lv: Some(60),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn hero(uid: i64) -> HeroInfo {
        HeroInfo {
            uid,
            hero_id: 3149,
            level: Some(60),
            rank: Some(5),
            talent: Some(10),
            talent_cube_infos: vec![TalentCubeInfo {
                cube_id: Some(61),
                ..Default::default()
            }],
            default_equip_uid: Some(100),
            ex_attr: Some(HeroExAttribute {
                cri: Some(i32::MAX),
                ..Default::default()
            }),
            sp_attr: Some(HeroSpAttribute {
                heal: Some(i32::MAX),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn write_roster(directory: &Path, hero: HeroInfo) {
        fs::write(
            directory.join("HeroInfoListReply_1.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    }

    fn write_request(directory: &Path, equips: Vec<sonettobuf::FightEquip>, is_balance: bool) {
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&sonettobuf::StartDungeonRequest {
                fight_group: Some(sonettobuf::FightGroup {
                    equips,
                    ..Default::default()
                }),
                is_balance: Some(is_balance),
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn captured_derived_attributes_are_not_runtime_inputs() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("derived-poison");
        let hero = hero(uid);
        let fight = fight(uid);
        write_roster(&directory, hero.clone());
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let build = preview_build_input(entity, &hero).unwrap();
        let equips = validated_equipment_loadout(entity, &hero, None).unwrap();
        let expected = Stats::build_for_loadout(&build, &equips);

        let (ex, sp) =
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap();

        assert_eq!(ex, vec![(uid, expected.ex())]);
        assert_eq!(sp, vec![(uid, expected.sp())]);
        assert_ne!(expected.cri, i32::MAX);
        assert_ne!(expected.heal, i32::MAX);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_request_equipment_overrides_the_roster_default() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("requested-equipment");
        let mut hero = hero(uid);
        hero.default_equip_uid = Some(999);
        write_roster(&directory, hero);
        write_request(
            &directory,
            vec![sonettobuf::FightEquip {
                hero_uid: Some(uid),
                equip_uid: vec![100],
                ..Default::default()
            }],
            false,
        );

        assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn request_and_entity_equipment_mismatch_fails_loudly() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("request-mismatch");
        write_roster(&directory, hero(uid));
        write_request(
            &directory,
            vec![sonettobuf::FightEquip {
                hero_uid: Some(uid),
                equip_uid: vec![999],
                ..Default::default()
            }],
            false,
        );

        assert!(
            preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("invalid equipment metadata")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn request_without_equipment_uses_the_roster_default() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("request-default");
        write_roster(&directory, hero(uid));
        write_request(&directory, Vec::new(), false);

        assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn negative_equipment_metadata_fails_loudly() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.equips.clear();
        entity.equip_uid = Some(-1);
        let mut hero = hero(uid);
        hero.default_equip_uid = Some(-1);

        assert!(validated_equipment_loadout(entity, &hero, None).is_err());
    }

    #[test]
    fn string_encoded_request_equipment_is_accepted() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("string-request-equipment");
        let mut hero = hero(uid);
        hero.default_equip_uid = Some(999);
        write_roster(&directory, hero);
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [{
                        "heroUid": uid.to_string(),
                        "equipUid": ["100"]
                    }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn snake_case_balance_metadata_is_preserved() {
        let directory = test_directory("snake-balance");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({ "is_balance": true })).unwrap(),
        )
        .unwrap();

        assert!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap()
                .is_balance
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn negative_request_equipment_fails_loudly() {
        let directory = test_directory("negative-request-equipment");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [{ "heroUid": 42, "equipUid": [-1] }]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("negative equipment uid")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn zero_owner_equipment_rows_are_validated_then_ignored() {
        let directory = test_directory("zero-owner-request");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [
                        { "heroUid": 0, "equipUid": [100] },
                        { "heroUid": 0, "equipUid": [0] },
                        { "heroUid": 42, "equipUid": [200] }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap()
                .selected_equips,
            HashMap::from([(42, 200)])
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn invalid_zero_owner_equipment_stays_fail_loud() {
        for (label, equip_uid) in [
            ("negative", serde_json::json!([-1])),
            ("malformed", serde_json::json!(["invalid"])),
        ] {
            let directory = test_directory(label);
            fs::write(
                directory.join("StartDungeonRequest.json"),
                serde_json::to_vec(&serde_json::json!({
                    "fightGroup": {
                        "equips": [{ "heroUid": 0, "equipUid": equip_uid }]
                    }
                }))
                .unwrap(),
            )
            .unwrap();

            assert!(battle_request_metadata(&directory.join("BeginRoundReply_1.json")).is_err());
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn duplicate_empty_and_selected_request_rows_fail_loudly() {
        let directory = test_directory("mixed-duplicate-request");
        fs::write(
            directory.join("StartDungeonRequest.json"),
            serde_json::to_vec(&serde_json::json!({
                "fightGroup": {
                    "equips": [
                        { "heroUid": 42, "equipUid": [] },
                        { "heroUid": 42, "equipUid": [100] }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(
            battle_request_metadata(&directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("duplicate equipment selection")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn duplicate_request_equipment_rows_fail_loudly() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("duplicate-request");
        write_roster(&directory, hero(uid));
        let selection = sonettobuf::FightEquip {
            hero_uid: Some(uid),
            equip_uid: vec![100],
            ..Default::default()
        };
        write_request(&directory, vec![selection.clone(), selection], false);

        assert!(
            preview_attributes(&fight(uid), &directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("duplicate equipment selection")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_build_metadata_fails_loudly() {
        crate::init_test_config();
        let directory = test_directory("missing-build");
        let error = preview_attributes(&fight(42), &directory.join("BeginRoundReply_1.json"))
            .unwrap_err()
            .to_string();

        assert!(error.contains("missing build metadata"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_talent_does_not_guess_a_default() {
        crate::init_test_config();
        let mut hero = hero(42);
        hero.talent = None;

        assert!(
            preview_build_input(&fight(42).attacker.unwrap().entitys[0], &hero)
                .unwrap_err()
                .to_string()
                .contains("missing talent")
        );
    }

    #[test]
    fn missing_rank_does_not_infer_from_level() {
        crate::init_test_config();
        let mut hero = hero(42);
        hero.rank = None;

        assert!(
            preview_build_input(&fight(42).attacker.unwrap().entitys[0], &hero)
                .unwrap_err()
                .to_string()
                .contains("missing rank")
        );
    }

    #[test]
    fn invalid_positive_trial_id_fails_loudly() {
        crate::init_test_config();
        let directory = test_directory("invalid-trial");
        let mut fight = fight(42);
        fight.attacker.as_mut().unwrap().entitys[0].trial_id = Some(i32::MAX);

        assert!(
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json"))
                .unwrap_err()
                .to_string()
                .contains("invalid trial")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn battle_balance_applies_to_the_full_equipment_loadout() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("balanced-loadout");
        let hero = hero(uid);
        write_roster(&directory, hero.clone());
        write_request(&directory, Vec::new(), true);
        let mut fight = fight(uid);
        fight.battle_id = Some(116385108);
        for equip in &mut fight.attacker.as_mut().unwrap().entitys[0].equips {
            equip.equip_lv = Some(50);
        }
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let build = preview_build_input(entity, &hero).unwrap();
        let equips = validated_equipment_loadout(entity, &hero, None).unwrap();
        let balance = request_battle_balance(&fight, true).unwrap().unwrap();
        let expected = balance.stats_for(&build, &equips);
        let unbalanced = Stats::build_for_loadout(&build, &equips);

        assert_eq!(
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
            (vec![(uid, expected.ex())], vec![(uid, expected.sp())]),
        );
        assert_ne!(expected, unbalanced);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn configured_trial_uses_configured_attributes_without_roster_metadata() {
        crate::init_test_config();
        let uid = 42;
        let trial_id = 116385001;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.trial_id = Some(trial_id);
        entity.equips.clear();
        entity.equip_uid = None;
        let directory = test_directory("trial");
        let (_, expected) =
            battle::engine::entity::builder::EntityBuilder::trial(trial_id, uid, 0, 0).unwrap();

        assert_eq!(
            preview_attributes(&fight, &directory.join("BeginRoundReply_1.json")).unwrap(),
            (vec![(uid, expected.ex())], vec![(uid, expected.sp())]),
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sorted_updates_override_roster_build_metadata() {
        crate::init_test_config();
        let uid = 42;
        let directory = test_directory("precedence");
        write_roster(&directory, hero(uid));
        let mut earlier = hero(uid);
        earlier.talent = Some(8);
        let mut later = hero(uid);
        later.talent = Some(12);
        fs::write(
            directory.join("HeroUpdatePush_2.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![later.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("HeroUpdatePush_1.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![earlier],
            })
            .unwrap(),
        )
        .unwrap();

        let heroes = battle_build_metadata(&directory.join("BeginRoundReply_1.json")).unwrap();

        assert_eq!(heroes[&uid].talent, later.talent);
        fs::remove_dir_all(directory).unwrap();
    }
}
