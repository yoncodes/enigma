use sonettobuf::CardInfo;

use crate::{catalog::BattleCatalog, engine::manager::BattleManagers};

pub fn temp_card(skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(0),
        skill_id: Some(skill_id),
        card_effect: None,
        temp_card: Some(true),
        enchants: Vec::new(),
        card_type: Some(0),
        hero_id: Some(0),
        status: Some(0),
        target_uid: Some(0),
        extra_info: None,
        energy: Some(0),
        extra_infos: Vec::new(),
        area_red_or_blue: Some(0),
        heat_id: Some(0),
        music_note: None,
        card_dataes: Vec::new(),
    }
}

pub fn precast_card(owner_uid: i64, skill_id: i32) -> CardInfo {
    let hero_id = BattleCatalog::try_global().and_then(|catalog| catalog.skill_hero_id(skill_id));
    build_precast_card(owner_uid, hero_id, skill_id)
}

pub(crate) fn resolve_precast_card(
    catalog: BattleCatalog,
    owner_uid: i64,
    skill_id: i32,
) -> CardInfo {
    build_precast_card(owner_uid, catalog.skill_hero_id(skill_id), skill_id)
}

pub(crate) fn runtime_precast_card(
    managers: &BattleManagers,
    owner_uid: i64,
    skill_id: i32,
) -> CardInfo {
    managers
        .try_catalog()
        .map(|catalog| resolve_precast_card(catalog, owner_uid, skill_id))
        .unwrap_or_else(|| precast_card(owner_uid, skill_id))
}

fn build_precast_card(owner_uid: i64, hero_id: Option<i32>, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(owner_uid),
        hero_id,
        card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
        ..temp_card(skill_id)
    }
}

pub fn selected_precast_card(owner_uid: i64, hero_id: i32, skill_id: i32) -> CardInfo {
    CardInfo {
        uid: Some(owner_uid),
        hero_id: Some(hero_id),
        ..temp_card(skill_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precast_hero_comes_from_the_selected_catalog() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::try_global().unwrap();

        assert_eq!(precast_card(10, 31345153).hero_id, Some(3134));
        assert_eq!(
            resolve_precast_card(catalog, 10, 31345153).hero_id,
            Some(3134)
        );
        assert_eq!(resolve_precast_card(catalog, 10, -1).hero_id, None);
    }
}
