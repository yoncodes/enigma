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

mod build;
mod request;
mod timeline;
