use sonettobuf::{CardHeatInfo, FightDeviceAreaInfo, FightEntityInfo, FightTeam, PlayerSkillInfo};

pub struct Team;

impl Team {
    pub fn build(
        entitys: Vec<FightEntityInfo>,
        sub_entitys: Vec<FightEntityInfo>,
        player_entity: FightEntityInfo,
        power: Option<i32>,
        cloth_id: Option<i32>,
        skill_infos: Vec<PlayerSkillInfo>,
    ) -> FightTeam {
        FightTeam {
            entitys,
            sub_entitys,
            power,
            cloth_id,
            skill_infos,
            sp_entitys: vec![],
            indicators: vec![],
            ex_team_str: Some(String::new()),
            assist_boss: None,
            assist_boss_info: None,
            emitter: None,
            emitter_info: None,
            player_entity: Some(player_entity),
            player_finisher_info: None,
            energy: Some(0),
            card_heat: Some(CardHeatInfo { values: vec![] }),
            card_deck_size: Some(0),
            blood_pool: None,
            vorpalith: None,
            item_skill_group: None,
            sp_fight_entities: vec![],
            heat_scale: None,
            music_info: None,
            device_card_deck_size: Some(0),
            device_area: Some(FightDeviceAreaInfo::default()),
        }
    }

    pub fn get_player_skills(cloth_id: Option<i32>) -> Vec<PlayerSkillInfo> {
        Self::player_skills(crate::catalog::BattleCatalog::global(), cloth_id)
    }

    pub(crate) fn player_skills(
        catalog: crate::catalog::BattleCatalog,
        cloth_id: Option<i32>,
    ) -> Vec<PlayerSkillInfo> {
        catalog
            .player_skills(cloth_id)
            .into_iter()
            .map(|skill| PlayerSkillInfo {
                skill_id: Some(skill.skill_id),
                cd: Some(0),
                need_power: skill.need_power,
                r#type: Some(0),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_configured_player_skills() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            Team::player_skills(catalog, Some(1)),
            Team::get_player_skills(Some(1))
        );
        assert_eq!(
            Team::player_skills(catalog, Some(1)),
            vec![
                PlayerSkillInfo {
                    skill_id: Some(30010201),
                    cd: Some(0),
                    need_power: Some(40),
                    r#type: Some(0),
                },
                PlayerSkillInfo {
                    skill_id: Some(30010202),
                    cd: Some(0),
                    need_power: Some(25),
                    r#type: Some(0),
                },
            ]
        );
    }
}
