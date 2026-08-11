use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use battle::engine::entity::{
    input::{EquipmentBuildInput, HeroBuildInput},
    stats::{StatInputs, Stats, rank_from_level},
};
use sonettobuf::{
    Fight, FightEntityInfo, HeroExAttribute, HeroInfo, HeroInfoListReply, HeroSpAttribute,
    HeroUpdatePush,
};

use crate::normalize_live_json;

type PreviewAttributes = (Vec<(i64, HeroExAttribute)>, Vec<(i64, HeroSpAttribute)>);

enum HeroMetadata {
    Roster(HeroInfo),
    Update(HeroInfo),
}

#[derive(Debug, PartialEq, Eq)]
enum MissingAttackerMetadataReason {
    RosterAttributes,
    SupplementalEquipment,
}

#[derive(Debug, PartialEq, Eq)]
struct MissingAttackerMetadata {
    uid: i64,
    model_id: i32,
    reason: MissingAttackerMetadataReason,
}

pub fn preview_attributes(fight: &Fight, battle_path: &Path) -> anyhow::Result<PreviewAttributes> {
    let local = battle_hero_metadata(battle_path)?;
    let (attributes, missing) = hydrate_preview_attributes(fight, &local);
    for missing in &missing {
        match &missing.reason {
            MissingAttackerMetadataReason::RosterAttributes => eprintln!(
                "attribute preview missing attacker metadata uid={} hero={}; using battle defaults",
                missing.uid, missing.model_id,
            ),
            MissingAttackerMetadataReason::SupplementalEquipment => eprintln!(
                "attribute preview incomplete supplemental equipment metadata uid={} hero={}; preserved roster attributes",
                missing.uid, missing.model_id,
            ),
        }
    }
    Ok(attributes)
}

fn hydrate_preview_attributes(
    fight: &Fight,
    local: &HashMap<i64, HeroMetadata>,
) -> (PreviewAttributes, Vec<MissingAttackerMetadata>) {
    let mut ex_attributes = Vec::new();
    let mut sp_attributes = Vec::new();
    let mut missing = Vec::new();
    for entity in fight
        .attacker
        .iter()
        .flat_map(|team| team.entitys.iter().chain(team.sub_entitys.iter()))
    {
        let Some(uid) = entity.uid else {
            continue;
        };
        match local.get(&uid) {
            Some(HeroMetadata::Roster(hero)) => {
                let RosterPreviewAttributes {
                    ex,
                    sp,
                    supplemental_rejected,
                } = roster_attributes(entity, hero);
                if let Some(attributes) = ex {
                    ex_attributes.push((uid, attributes));
                }
                if let Some(attributes) = sp {
                    sp_attributes.push((uid, attributes));
                }
                if hero.ex_attr.is_none() || hero.sp_attr.is_none() {
                    missing.push(MissingAttackerMetadata {
                        uid,
                        model_id: entity.model_id.unwrap_or_default(),
                        reason: MissingAttackerMetadataReason::RosterAttributes,
                    });
                } else if supplemental_rejected {
                    missing.push(MissingAttackerMetadata {
                        uid,
                        model_id: entity.model_id.unwrap_or_default(),
                        reason: MissingAttackerMetadataReason::SupplementalEquipment,
                    });
                } else if battle::engine::diagnostics::enabled(
                    battle::engine::diagnostics::TraceArea::Damage,
                ) {
                    eprintln!(
                        "attribute preview uid={uid} hero={} source=hero-roster",
                        entity.model_id.unwrap_or_default(),
                    );
                }
            }
            Some(HeroMetadata::Update(hero)) => {
                let inputs = preview_stat_inputs(entity, hero);
                let stats = Stats::build(&inputs);
                if battle::engine::diagnostics::enabled(
                    battle::engine::diagnostics::TraceArea::Damage,
                ) {
                    eprintln!(
                        "attribute preview uid={uid} hero={} source=hero-update inputs={inputs:?} stats={stats:?}",
                        entity.model_id.unwrap_or_default(),
                    );
                }
                ex_attributes.push((uid, stats.ex()));
                sp_attributes.push((uid, stats.sp()));
            }
            None => missing.push(MissingAttackerMetadata {
                uid,
                model_id: entity.model_id.unwrap_or_default(),
                reason: MissingAttackerMetadataReason::RosterAttributes,
            }),
        }
    }
    ((ex_attributes, sp_attributes), missing)
}

#[derive(Debug, PartialEq, Eq)]
struct RosterPreviewAttributes {
    ex: Option<HeroExAttribute>,
    sp: Option<HeroSpAttribute>,
    supplemental_rejected: bool,
}

fn roster_attributes(entity: &FightEntityInfo, hero: &HeroInfo) -> RosterPreviewAttributes {
    let (break_stats, supplemental_rejected) = match supplemental_equipment_stats(entity, hero) {
        Ok(stats) => (stats, false),
        Err(()) => (Stats::default(), true),
    };
    let ex = hero.ex_attr.map(|mut attributes| {
        add_stat(&mut attributes.cri, break_stats.cri);
        add_stat(&mut attributes.recri, break_stats.recri);
        add_stat(&mut attributes.cri_dmg, break_stats.cri_dmg);
        add_stat(&mut attributes.cri_def, break_stats.cri_def);
        add_stat(&mut attributes.add_dmg, break_stats.add_dmg);
        add_stat(&mut attributes.drop_dmg, break_stats.drop_dmg);
        attributes
    });
    let sp = hero.sp_attr.map(|mut attributes| {
        add_stat(&mut attributes.revive, break_stats.revive);
        add_stat(&mut attributes.heal, break_stats.heal);
        add_stat(&mut attributes.absorb, break_stats.absorb);
        add_stat(&mut attributes.defense_ignore, break_stats.defense_ignore);
        add_stat(&mut attributes.clutch, break_stats.clutch);
        add_stat(
            &mut attributes.normal_skill_rate,
            break_stats.normal_skill_rate,
        );
        add_stat(&mut attributes.rebound_dmg, break_stats.rebound_dmg);
        add_stat(&mut attributes.extra_dmg, break_stats.extra_dmg);
        add_stat(&mut attributes.reuse_dmg, break_stats.reuse_dmg);
        attributes
    });
    RosterPreviewAttributes {
        ex,
        sp,
        supplemental_rejected,
    }
}

fn supplemental_equipment_stats(entity: &FightEntityInfo, hero: &HeroInfo) -> Result<Stats, ()> {
    if entity.equips.is_empty() {
        return match (hero.default_equip_uid, entity.equip_uid) {
            (None | Some(0), None | Some(0)) => Ok(Stats::default()),
            _ => Err(()),
        };
    }
    if entity.model_id.filter(|model_id| *model_id > 0) != Some(hero.hero_id) {
        return Err(());
    }
    let default_equip_uid = hero.default_equip_uid.filter(|uid| *uid > 0).ok_or(())?;
    let selected_equip_uid = entity.equip_uid.filter(|uid| *uid > 0).ok_or(())?;
    if selected_equip_uid != default_equip_uid {
        return Err(());
    }
    let primary = entity.equips.first().ok_or(())?;
    if primary.equip_uid != Some(selected_equip_uid) {
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

    if entity.equips.len() > 2 {
        return Err(());
    }
    let linked_equip_id = game.linked_psychube_id(hero.hero_id, primary_equip_id);
    let equips = entity
        .equips
        .get(1)
        .map(|equip| {
            let uid = equip.equip_uid.ok_or(())?;
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
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    Ok(Stats::build_for_loadout(
        &HeroBuildInput::default(),
        &equips,
    ))
}

fn add_stat(value: &mut Option<i32>, addition: i32) {
    if let Some(value) = value {
        *value += addition;
    }
}

fn battle_hero_metadata(path: &Path) -> anyhow::Result<HashMap<i64, HeroMetadata>> {
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
            heroes.insert(hero.uid, HeroMetadata::Roster(hero));
        }
    }
    for file in update_files {
        let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(file)?)?;
        normalize_live_json(&mut value);
        let update: HeroUpdatePush = serde_json::from_value(value)?;
        for hero in update.hero_updates {
            heroes.insert(hero.uid, HeroMetadata::Update(hero));
        }
    }
    Ok(heroes)
}

fn preview_stat_inputs(entity: &FightEntityInfo, hero: &HeroInfo) -> StatInputs {
    let selected_template = hero.use_talent_template_id.unwrap_or_default();
    let template = (selected_template != 0)
        .then(|| {
            hero.talent_templates
                .iter()
                .find(|template| template.id == Some(selected_template))
        })
        .flatten();
    let cubes = template
        .filter(|template| !template.talent_cube_infos.is_empty())
        .map(|template| template.talent_cube_infos.as_slice())
        .unwrap_or(&hero.talent_cube_infos);
    let equip = entity.equips.first();
    StatInputs {
        hero_id: entity.model_id.unwrap_or(hero.hero_id),
        level: entity.level.or(hero.level).unwrap_or_default(),
        rank: hero.rank.unwrap_or_else(|| {
            rank_from_level(
                entity.model_id.unwrap_or(hero.hero_id),
                entity.level.unwrap_or_default(),
            )
        }),
        destiny_rank: entity
            .destiny_rank
            .or(hero.destiny_rank)
            .unwrap_or_default(),
        equip_id: equip.and_then(|equip| equip.equip_id).unwrap_or_default(),
        equip_level: equip.and_then(|equip| equip.equip_lv).unwrap_or_default(),
        equip_break_level: 0,
        talent: hero.talent.unwrap_or(10),
        talent_style: template
            .and_then(|template| template.style)
            .unwrap_or_default(),
        talent_placements: cubes.iter().filter_map(|cube| cube.cube_id).collect(),
    }
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
                    passive_skill: vec![437111, 437215],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn hero(uid: i64, critical_damage: i32) -> HeroInfo {
        HeroInfo {
            uid,
            user_id: 1,
            hero_id: 3149,
            ex_attr: Some(HeroExAttribute {
                cri: Some(385),
                recri: Some(100),
                cri_dmg: Some(critical_damage),
                cri_def: Some(0),
                add_dmg: Some(85),
                drop_dmg: Some(45),
            }),
            sp_attr: Some(HeroSpAttribute {
                device_skill_rate: Some(70),
                ..Default::default()
            }),
            default_equip_uid: Some(100),
            ..Default::default()
        }
    }

    #[test]
    fn roster_fallback_keeps_default_equip_and_adds_supplemental_break_attributes() {
        crate::init_test_config();
        let directory = test_directory("roster");
        let uid = 42;
        fs::write(
            directory.join("HeroInfoListReply_1.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid, 1485)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        let battle_path = directory.join("BeginRoundReply_1.json");
        let fight = fight(uid);

        let (extended, special) = preview_attributes(&fight, &battle_path).unwrap();

        let mut expected = hero(uid, 1485).ex_attr.unwrap();
        expected.cri_dmg = Some(1725);
        assert_eq!(extended, vec![(uid, expected)]);
        assert_eq!(special, vec![(uid, hero(uid, 1485).sp_attr.unwrap())]);
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        assert_eq!(entity.equips.len(), 2);
        assert_eq!(entity.passive_skill, vec![437111, 437215]);
        fs::remove_dir_all(directory).unwrap();
    }

    fn assert_roster_attributes_preserved(fight: &Fight, hero: &HeroInfo) {
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let attributes = roster_attributes(entity, hero);
        assert_eq!((attributes.ex, attributes.sp), (hero.ex_attr, hero.sp_attr));
        assert!(attributes.supplemental_rejected);
    }

    #[test]
    fn roster_fallback_rejects_missing_default_equip_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut roster = hero(uid, 1485);
        roster.default_equip_uid = None;

        assert_roster_attributes_preserved(&fight(uid), &roster);
    }

    #[test]
    fn roster_fallback_rejects_missing_selected_primary_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equip_uid = None;

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_zero_selected_primary_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equip_uid = Some(0);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_mismatched_selected_primary_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equip_uid = Some(300);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_missing_or_mismatched_model_id() {
        crate::init_test_config();
        let uid = 42;

        for model_id in [None, Some(3028)] {
            let mut fight = fight(uid);
            fight.attacker.as_mut().unwrap().entitys[0].model_id = model_id;

            assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
        }
    }

    #[test]
    fn roster_fallback_rejects_reordered_primary_equip() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0]
            .equips
            .swap(0, 1);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_missing_fight_equip_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_uid = None;

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_zero_supplemental_equip_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_uid = Some(0);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_zero_default_equip_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut roster = hero(uid, 1485);
        roster.default_equip_uid = Some(0);

        assert_roster_attributes_preserved(&fight(uid), &roster);
    }

    #[test]
    fn empty_fight_loadout_with_absent_or_zero_default_has_no_supplemental_attributes() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.equips.clear();

        for (default_equip_uid, selected_equip_uid) in [(None, None), (Some(0), Some(0))] {
            entity.equip_uid = selected_equip_uid;
            let mut roster = hero(uid, 1485);
            roster.default_equip_uid = default_equip_uid;
            let attributes = roster_attributes(entity, &roster);

            assert_eq!(
                (attributes.ex, attributes.sp),
                (roster.ex_attr, roster.sp_attr)
            );
            assert!(!attributes.supplemental_rejected);
        }
    }

    #[test]
    fn empty_fight_loadout_rejects_selected_equipment_without_a_record() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.equips.clear();
        let mut roster = hero(uid, 1485);
        roster.default_equip_uid = None;

        assert_roster_attributes_preserved(&fight, &roster);
    }

    #[test]
    fn roster_fallback_accepts_default_only_without_a_linked_companion() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        entity.model_id = Some(3028);
        entity.equips.truncate(1);
        let mut roster = hero(uid, 1485);
        roster.hero_id = 3028;

        let attributes = roster_attributes(entity, &roster);

        assert_eq!(config::configs::get().linked_psychube_id(3028, 1571), None);
        assert_eq!(
            (attributes.ex, attributes.sp),
            (roster.ex_attr, roster.sp_attr)
        );
        assert!(!attributes.supplemental_rejected);
    }

    #[test]
    fn roster_fallback_rejects_invalid_primary_configuration() {
        crate::init_test_config();
        let uid = 42;

        for (equip_id, equip_lv) in [
            (None, Some(60)),
            (Some(999999), Some(60)),
            (Some(1571), None),
            (Some(1571), Some(61)),
        ] {
            let mut fight = fight(uid);
            let primary = &mut fight.attacker.as_mut().unwrap().entitys[0].equips[0];
            primary.equip_id = equip_id;
            primary.equip_lv = equip_lv;

            assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
        }
    }

    #[test]
    fn roster_fallback_rejects_missing_supplemental_equip_id() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_id = None;

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_missing_supplemental_level() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_lv = None;

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_unknown_supplemental_equip_id() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_id = Some(999999);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_unlinked_supplemental_equip() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_id = Some(1501);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_excess_supplemental_equips() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0]
            .equips
            .push(EquipRecord {
                equip_uid: Some(300),
                equip_id: Some(1501),
                equip_lv: Some(60),
                ..Default::default()
            });

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_out_of_range_supplemental_level() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_lv = Some(61);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn roster_fallback_rejects_duplicate_equip_uid() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_uid = Some(100);

        assert_roster_attributes_preserved(&fight, &hero(uid, 1485));
    }

    #[test]
    fn partial_roster_attributes_add_present_break_fields_only() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
        let mut roster = hero(uid, 1485);
        roster.ex_attr = Some(HeroExAttribute {
            cri_dmg: Some(1485),
            ..Default::default()
        });
        roster.sp_attr = Some(HeroSpAttribute::default());

        let RosterPreviewAttributes {
            ex,
            sp,
            supplemental_rejected,
        } = roster_attributes(entity, &roster);
        let ex = ex.unwrap();
        let sp = sp.unwrap();
        assert!(!supplemental_rejected);
        assert_eq!(ex.cri_dmg, Some(1725));
        assert_eq!(ex.cri, None);
        assert_eq!(sp.clutch, None);
        assert_eq!(sp.heal, None);
        assert_eq!(sp.device_skill_rate, None);
    }

    #[test]
    fn sorted_hero_updates_override_roster_metadata() {
        crate::init_test_config();
        let directory = test_directory("precedence");
        let uid = 42;
        let mut earlier_update = hero(uid, 1666);
        earlier_update.level = Some(1);
        earlier_update.rank = Some(0);
        earlier_update.talent_cube_infos = vec![TalentCubeInfo {
            cube_id: Some(62),
            ..Default::default()
        }];
        let mut later_update = hero(uid, 1777);
        later_update.level = Some(60);
        later_update.rank = Some(5);
        later_update.talent_cube_infos = vec![TalentCubeInfo {
            cube_id: Some(61),
            ..Default::default()
        }];
        fs::write(
            directory.join("HeroInfoListReply_1.json"),
            serde_json::to_vec(&HeroInfoListReply {
                heros: vec![hero(uid, 1485)],
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("HeroUpdatePush_2.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![later_update.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("HeroUpdatePush_1.json"),
            serde_json::to_vec(&HeroUpdatePush {
                hero_updates: vec![earlier_update.clone()],
            })
            .unwrap(),
        )
        .unwrap();

        let fight = fight(uid);
        let battle_path = directory.join("BeginRoundReply_1.json");
        let (extended, special) = preview_attributes(&fight, &battle_path).unwrap();
        let entity = &fight.attacker.as_ref().unwrap().entitys[0];
        let expected = Stats::build(&preview_stat_inputs(entity, &later_update));
        let earlier = Stats::build(&preview_stat_inputs(entity, &earlier_update));

        assert_eq!(extended, vec![(uid, expected.ex())]);
        assert_eq!(special, vec![(uid, expected.sp())]);
        assert_ne!(extended, vec![(uid, earlier.ex())]);
        assert_ne!(extended, vec![(uid, hero(uid, 1485).ex_attr.unwrap())]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_attacker_metadata_is_reported_without_failing() {
        let uid = 42;
        let (attributes, missing) = hydrate_preview_attributes(&fight(uid), &HashMap::new());

        assert_eq!(attributes, (Vec::new(), Vec::new()));
        assert_eq!(
            missing,
            vec![MissingAttackerMetadata {
                uid,
                model_id: 3149,
                reason: MissingAttackerMetadataReason::RosterAttributes,
            }]
        );
    }

    #[test]
    fn rejected_supplemental_metadata_is_reported_without_using_battle_defaults() {
        crate::init_test_config();
        let uid = 42;
        let mut fight = fight(uid);
        fight.attacker.as_mut().unwrap().entitys[0].equips[1].equip_lv = None;
        let hero = hero(uid, 1485);
        let mut local = HashMap::new();
        local.insert(uid, HeroMetadata::Roster(hero.clone()));

        let (attributes, missing) = hydrate_preview_attributes(&fight, &local);

        assert_eq!(
            attributes,
            (
                vec![(uid, hero.ex_attr.unwrap())],
                vec![(uid, hero.sp_attr.unwrap())]
            )
        );
        assert_eq!(
            missing,
            vec![MissingAttackerMetadata {
                uid,
                model_id: 3149,
                reason: MissingAttackerMetadataReason::SupplementalEquipment,
            }]
        );
    }

    #[cfg(feature = "private-fixtures")]
    #[test]
    fn battle_local_talent_placements_reconstruct_captured_attributes() {
        crate::init_test_config();
        let battle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/battles/battle2/begin_round_1.json");
        let heroes = battle_hero_metadata(&battle).unwrap();
        let mut start: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(battle.with_file_name("StartDungeonReply.json")).unwrap(),
        )
        .unwrap();
        normalize_live_json(&mut start);
        let fight: Fight = serde_json::from_value(start["fight"].clone()).unwrap();

        for entity in fight.attacker.iter().flat_map(|team| &team.entitys) {
            let uid = entity.uid.unwrap();
            let Some(HeroMetadata::Update(hero)) = heroes.get(&uid) else {
                continue;
            };
            let stats = Stats::build(&preview_stat_inputs(entity, hero));
            let generated = stats.ex();
            let captured = hero.ex_attr.as_ref().unwrap();
            let generated_sp = stats.sp();
            let captured_sp = hero.sp_attr.as_ref().unwrap();
            let base = entity.attr.as_ref().unwrap();

            assert_eq!(stats.hp, base.hp.unwrap());
            assert_eq!(stats.atk, base.attack.unwrap());
            assert_eq!(stats.def, base.defense.unwrap());
            assert_eq!(stats.mdef, base.mdefense.unwrap());
            assert_eq!(stats.technic, base.technic.unwrap());

            assert_eq!(generated.cri, captured.cri);
            assert_eq!(generated.recri, captured.recri);
            assert_eq!(generated.cri_dmg, captured.cri_dmg);
            assert_eq!(generated.cri_def, captured.cri_def);
            assert_eq!(generated.add_dmg, captured.add_dmg);
            assert_eq!(generated.drop_dmg, captured.drop_dmg);
            // HeroUpdate does not fold destiny-stone attributes into SpAttr.
            if entity.destiny_rank.unwrap_or_default() == 0 {
                assert_eq!(
                    (
                        generated_sp.revive,
                        generated_sp.heal,
                        generated_sp.absorb,
                        generated_sp.defense_ignore,
                        generated_sp.clutch,
                        generated_sp.normal_skill_rate,
                        generated_sp.rebound_dmg,
                        generated_sp.extra_dmg,
                        generated_sp.reuse_dmg,
                    ),
                    (
                        captured_sp.revive,
                        captured_sp.heal,
                        captured_sp.absorb,
                        captured_sp.defense_ignore,
                        captured_sp.clutch,
                        captured_sp.normal_skill_rate,
                        captured_sp.rebound_dmg,
                        captured_sp.extra_dmg,
                        captured_sp.reuse_dmg,
                    ),
                    "special attributes differ for uid={uid}",
                );
            }
        }
    }

    #[cfg(feature = "private-fixtures")]
    #[test]
    fn tutorial_trial_without_a_hero_update_does_not_invent_extended_attributes() {
        crate::init_test_config();
        let battle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/battles/battle62/BeginRoundReply_1.json");
        let mut start: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(battle.with_file_name("StartDungeonReply.json")).unwrap(),
        )
        .unwrap();
        normalize_live_json(&mut start);
        let fight: Fight = serde_json::from_value(start["fight"].clone()).unwrap();

        let (extended, special) = preview_attributes(&fight, &battle).unwrap();

        assert!(extended.is_empty());
        assert!(special.is_empty());
    }
}
