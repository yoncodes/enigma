//! Selected Tides of Thought components, captured when a fight starts.

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sonettobuf::{CustomData, Fight, FightEntityInfo, FightTeam, HeroExAttribute, TowerComposeMods};

use crate::{catalog::BattleCatalog, engine::fight::defender::Defender};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeFightData {
    pub theme_id: i32,
    pub layer_id: i32,
    pub plane_id: i32,
    pub max_plane_id: i32,
    #[serde(default)]
    pub research_id: i32,
    #[serde(default)]
    pub mods: Vec<TowerComposeMods>,
}

impl ComposeFightData {
    pub fn mod_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.mods.iter().flat_map(|group| &group.mods)
            .filter_map(|item| item.mod_id.filter(|id| *id > 0))
    }

    pub fn from_fight(fight: &Fight) -> Option<Self> {
        let kind = sonettobuf::custom_data::CustomDataType::TowerCompose as i32;
        let raw = fight.custom_data.iter().find(|data| data.r#type == Some(kind))?.data.as_deref()?;
        serde_json::from_str(raw).ok()
    }
}

/// The zero-requirement research uses a level table instead of additionRule.
/// Its damage bonus applies only to the boss plane and falls to zero beyond
/// the last configured level. The other research choices use normal rules.
pub(crate) fn default_research_skills(db: &config::GameDB, data: &ComposeFightData) -> Vec<i32> {
    let selected = db.tower_compose_research.get(data.research_id).is_some_and(|research| {
        research.theme_id == data.theme_id && research.req == 0 && research.addition_rule.trim().is_empty()
    });
    if data.plane_id <= 0 || !selected {
        return Vec::new();
    }
    let level = data.mod_ids().filter_map(|id| db.tower_compose_mod.get(id))
        .filter(|item| item.theme_id == data.theme_id)
        .fold(0_i32, |sum, item| sum.saturating_add(item.level)).max(1);
    db.tower_compose_const.get(11).and_then(|row| {
        row.value.split('|').filter_map(|entry| {
            let (entry_level, skill) = entry.split_once('#')?;
            Some((entry_level.trim().parse::<i32>().ok()?, skill.trim().parse::<i32>().ok()?))
        }).find_map(|(entry_level, skill)| (entry_level == level).then_some(skill))
    }).filter(|id| db.skill.get(*id).is_some()).into_iter().collect()
}

pub fn apply_to_built(catalog: BattleCatalog, built: &mut super::BuiltFight, data: &ComposeFightData) -> Result<()> {
    let mut plain = built.fight.clone();
    let kind = sonettobuf::custom_data::CustomDataType::TowerCompose as i32;
    plain.custom_data.retain(|item| item.r#type != Some(kind));
    let old_rules = crate::engine::fight::rules::configured_with_catalog(catalog, &plain);
    if let Some(team) = built.fight.attacker.as_mut() {
        remove_rules(team, 1, &old_rules);
    }
    if let Some(team) = built.fight.defender.as_mut() {
        remove_rules(team, 2, &old_rules);
    }
    built.fight.custom_data.retain(|item| item.r#type != Some(kind));
    built.fight.custom_data.push(CustomData { r#type: Some(kind), data: Some(serde_json::to_string(data)?) });
    if let Some(team) = built.fight.defender.as_mut() {
        for entity in team.entitys.iter_mut().chain(&mut team.sub_entitys)
            .chain(&mut team.sp_entitys).chain(&mut team.sp_fight_entities)
        {
            if let Some(ex) = apply_entity(catalog, data, built.fight.episode_id.unwrap_or_default(), entity)? {
                built.ex_attributes.push((entity.uid.context("compose monster has no uid")?, ex));
            }
        }
    }
    let rules = crate::engine::fight::rules::configured_with_catalog(catalog, &built.fight);
    if let Some(team) = built.fight.attacker.as_mut() { add_rules(team, 1, &rules); }
    if let Some(team) = built.fight.defender.as_mut() { add_rules(team, 2, &rules); }
    Ok(())
}

fn side_applies(side: crate::engine::fight::rules::BattleRuleSide, team: i32) -> bool {
    use crate::engine::fight::rules::BattleRuleSide::*;
    matches!((side, team), (Both, _) | (Attacker, 1) | (Defender, 2))
}

fn rule_skills(rules: &[crate::engine::fight::rules::ConfiguredBattleRule], team: i32) -> Vec<i32> {
    use crate::engine::fight::rules::AdditionRuleType;
    rules.iter().filter(|rule| rule.rule_type != AdditionRuleType::FightSkill && side_applies(rule.side, team))
        .map(|rule| rule.skill_id).collect()
}

fn remove_rules(team: &mut FightTeam, side: i32, rules: &[crate::engine::fight::rules::ConfiguredBattleRule]) {
    let skills = rule_skills(rules, side);
    for entity in team.entitys.iter_mut().chain(&mut team.sub_entitys)
        .chain(&mut team.sp_entitys).chain(&mut team.sp_fight_entities)
    {
        for skill in &skills {
            let index = if side == 1 { entity.passive_skill.iter().rposition(|id| id == skill) }
                else { entity.passive_skill.iter().position(|id| id == skill) };
            if let Some(index) = index { entity.passive_skill.remove(index); }
        }
    }
}

pub(crate) fn add_rules(team: &mut FightTeam, side: i32, rules: &[crate::engine::fight::rules::ConfiguredBattleRule]) {
    let skills = rule_skills(rules, side);
    for entity in team.entitys.iter_mut().chain(&mut team.sub_entitys)
        .chain(&mut team.sp_entitys).chain(&mut team.sp_fight_entities)
    {
        if side == 1 { entity.passive_skill.extend(skills.iter().copied()); }
        else { entity.passive_skill.splice(0..0, skills.iter().copied()); }
    }
}

pub(crate) fn apply_entity(
    catalog: BattleCatalog, data: &ComposeFightData, episode_id: i32, entity: &mut FightEntityInfo,
) -> Result<Option<HeroExAttribute>> {
    let game = catalog.game_data();
    let mods = data.mod_ids().map(|id| game.tower_compose_mod.get(id)
        .filter(|item| item.theme_id == data.theme_id)
        .with_context(|| format!("unknown compose component {id}"))).collect::<Result<Vec<_>>>()?;
    for item in &mods {
        if let Some((old, new)) = parse_pair(&item.monster_change, '#')? {
            if entity.model_id == Some(old) {
                *entity = Defender::build_monster(catalog, new,
                    entity.uid.context("compose monster has no uid")?,
                    entity.position.context("compose monster has no position")?,
                    entity.team_type.unwrap_or(2))?;
            }
        }
    }
    let model = entity.model_id.unwrap_or_default();
    for item in &mods {
        for entry in item.skill_change.split('|').filter(|part| !part.trim().is_empty()) {
            let (owner, change) = entry.split_once(':').context("invalid compose skill change")?;
            let (old, new) = parse_pair(change, '#')?.context("missing compose skill change")?;
            if owner.parse::<i32>()? == model {
                for skill in entity.skill_group1.iter_mut().chain(&mut entity.skill_group2).chain(&mut entity.passive_skill) {
                    if *skill == old { *skill = new; }
                }
                if entity.ex_skill == Some(old) { entity.ex_skill = Some(new); }
            }
        }
        for entry in item.passive_skill_add.split('|').filter(|part| !part.trim().is_empty()) {
            let (owner, skill) = entry.split_once(':').context("invalid compose passive")?;
            if owner.parse::<i32>()? == model { entity.passive_skill.push(skill.parse()?); }
        }
        for entry in item.career_change.split('|').filter(|part| !part.trim().is_empty()) {
            let (owner, career) = entry.split_once(':').context("invalid compose career")?;
            if owner.parse::<i32>()? == model { entity.career = Some(career.parse()?); }
        }
    }
    let level = mods.iter().try_fold(0_i32, |sum, item| sum.checked_add(item.level))
        .context("compose level overflow")?;
    let Some(boss_lv) = game.tower_compose_boss_lv.iter()
        .filter(|row| row.episode_id == episode_id && row.level <= level).max_by_key(|row| row.level)
    else { return Ok(None); };
    let Some(monster) = game.monster.get(model) else { return Ok(None); };
    let template = if monster.template != 0 { monster.template } else { model };
    for entry in boss_lv.template_change.split('|').filter(|part| !part.trim().is_empty()) {
        let (old, new) = parse_pair(entry, '#')?.context("missing compose template change")?;
        if template == old {
            let stats = crate::engine::entity::stats::configured_template_stats(game, new, entity.level.unwrap_or_default())
                .context("missing compose monster template")?;
            let base = stats.base();
            ensure!(base.hp.unwrap_or_default() > 0, "invalid compose monster health");
            entity.current_hp = base.hp;
            entity.attr = Some(base);
            entity.base_attr = Some(base);
            return Ok(Some(stats.ex()));
        }
    }
    Ok(None)
}

fn parse_pair(raw: &str, separator: char) -> Result<Option<(i32, i32)>> {
    if raw.trim().is_empty() { return Ok(None); }
    let (first, second) = raw.split_once(separator).context("invalid compose pair")?;
    Ok(Some((first.trim().parse()?, second.trim().parse()?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{TowerComposeMod, TowerComposeMods};

    fn selected(ids: &[i32]) -> ComposeFightData {
        ComposeFightData {
            theme_id: 1, layer_id: 6, plane_id: 1, max_plane_id: 2, research_id: 1401,
            mods: vec![TowerComposeMods {
                r#type: Some(1),
                mods: ids.iter().enumerate().map(|(index, id)| TowerComposeMod {
                    slot: Some(index as i32 + 1), mod_id: Some(*id),
                }).collect(),
            }],
        }
    }

    #[test]
    fn default_research_uses_boss_level_table_only_when_selected() {
        crate::test_support::init_config();
        let db = crate::test_support::game_data();
        let catalog = BattleCatalog::new(db);
        let mut data = selected(&[11601]); // level 5: 50% damage and reduction
        data.research_id = 1400;
        assert_eq!(default_research_skills(db, &data), vec![23390265]);
        data.mods = selected(&[11601, 11602, 11204]).mods; // level 12: 45%
        assert_eq!(default_research_skills(db, &data), vec![23390272]);
        data.mods = selected(&[11601, 11602, 11603, 11601, 11101]).mods; // level 21: 0%
        assert!(default_research_skills(db, &data).is_empty());
        data.plane_id = 0;
        assert!(default_research_skills(db, &data).is_empty());
        data.plane_id = 1;
        data.research_id = 1401;
        assert!(default_research_skills(db, &data).is_empty());

        data.research_id = 1400;
        data.mods = selected(&[11601]).mods;
        let mut built = super::super::BuiltFight {
            fight: Fight {
                episode_id: Some(90400199), battle_id: Some(9001001),
                attacker: Some(FightTeam {
                    entitys: vec![FightEntityInfo { uid: Some(10), current_hp: Some(100), ..Default::default() }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ex_attributes: vec![], sp_attributes: vec![], battle_rule_skills: vec![],
        };
        apply_to_built(catalog, &mut built, &data).unwrap();
        let passive = &built.fight.attacker.as_ref().unwrap().entitys[0].passive_skill;
        assert!(passive.contains(&23390265));
        let skills = crate::engine::skill::effect::SkillEffectCatalog::from_fight(db, &built.fight);
        assert!(skills.get(23390265).is_some());
    }

    #[test]
    fn configured_components_change_monsters_skills_level_and_rules() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());
        let data = selected(&[11101, 11204, 11305, 11415, 11602]);
        let mut core = Defender::build_monster(catalog, 900100101, -1, 1, 2).unwrap();
        let ex = apply_entity(catalog, &data, 90400199, &mut core).unwrap();
        assert!(ex.is_some());
        assert_eq!(core.current_hp, Some(5_040_000));
        assert_eq!(core.career, Some(5));
        assert!(core.passive_skill.contains(&23410005));
        assert!(core.passive_skill.contains(&23390113));
        assert!(!core.passive_skill.contains(&23390112));

        let mut arm = Defender::build_monster(catalog, 900100301, -2, 2, 2).unwrap();
        apply_entity(catalog, &data, 90400199, &mut arm).unwrap();
        assert_eq!(arm.model_id, Some(900100302));
        assert_eq!(arm.uid, Some(-2));
        assert!(arm.skill_group1.contains(&23310214));
        assert!(arm.skill_group2.contains(&23310215));
        assert!(arm.passive_skill.contains(&23310216));

        let mut built = super::super::BuiltFight {
            fight: Fight { episode_id: Some(90400199), battle_id: Some(9001001), ..Default::default() },
            ex_attributes: vec![], sp_attributes: vec![], battle_rule_skills: vec![],
        };
        apply_to_built(catalog, &mut built, &data).unwrap();
        let rules = crate::engine::fight::rules::configured_with_catalog(catalog, &built.fight);
        assert!(!rules.iter().any(|rule| rule.rule_id == 23390111));
        assert!(rules.iter().any(|rule| rule.rule_id == 23390121));
        assert!(rules.iter().any(|rule| rule.rule_id == 23300001));
        assert!(rules.iter().any(|rule| rule.rule_id == 113330002));
        let skills = crate::engine::skill::effect::SkillEffectCatalog::from_fight(
            crate::test_support::game_data(), &built.fight,
        );
        assert!(skills.get(23300001).is_some());
        assert!(skills.get(23390121).is_some());
    }

    #[test]
    fn second_wave_keeps_selected_components_and_template() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());
        let defender = Defender::configured(catalog, 9001001, 0).unwrap();
        let mut built = super::super::BuiltFight {
            fight: Fight {
                episode_id: Some(90400199), battle_id: Some(9001001), cur_wave: Some(1),
                attacker: Some(FightTeam::default()), defender: Some(defender.team),
                ..Default::default()
            },
            ex_attributes: vec![], sp_attributes: vec![], battle_rule_skills: vec![],
        };
        let data = selected(&[11101, 11204, 11305, 11415, 11602]);
        apply_to_built(catalog, &mut built, &data).unwrap();
        let mut fight = built.fight;
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        let wave = managers.advance_wave(&mut fight).unwrap().unwrap();
        let entities = &wave.fight.defender.unwrap().entitys;
        let core = entities.iter().find(|entity| entity.model_id == Some(900100101)).unwrap();
        assert_eq!(core.current_hp, Some(5_040_000));
        assert!(core.passive_skill.contains(&23410005));
        assert!(entities.iter().any(|entity| entity.model_id == Some(900100302)));
        assert!(entities.iter().any(|entity| entity.model_id == Some(900100403)));
    }
}
