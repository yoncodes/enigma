use super::closure::{
    apply_destiny, configured_skill_family_ids, configured_skill_ids, enqueue,
    enqueue_monster_skills,
};
use super::*;

#[derive(Debug)]
pub(crate) struct Pending {
    pub(crate) id: i32,
    pub(crate) path: String,
}

pub(crate) fn collect_hero_roots(
    options: &Options,
    hero_id: i32,
    db: &config::GameDB,
    skills: &mut VecDeque<Pending>,
    report: &mut Report,
) -> Result<()> {
    if let Some((stone, rank)) = options.destiny_stone.zip(options.destiny_rank) {
        println!("destiny_stone={stone} rank={rank}");
    }
    if let Some((psychube_id, level)) = options.psychube_id.zip(options.psychube_level) {
        println!("psychube={psychube_id} skill_rank={level}");
    }
    collect_hero_build_roots(
        hero_id,
        options.psychube_id.zip(options.psychube_level),
        options.destiny_stone.zip(options.destiny_rank),
        db,
        skills,
        report,
    )
}

pub(crate) fn collect_hero_build_roots(
    hero_id: i32,
    psychube: Option<(i32, i32)>,
    destiny_selection: Option<(i32, i32)>,
    db: &config::GameDB,
    skills: &mut VecDeque<Pending>,
    report: &mut Report,
) -> Result<()> {
    let hero = db
        .character
        .get(hero_id)
        .with_context(|| format!("hero {hero_id} is missing from character config"))?;

    let destiny = if let Some((stone, rank)) = destiny_selection {
        let choices = Destiny::stones(db, hero_id);
        if !choices.contains(&stone) {
            bail!("destiny stone {stone} is not available to hero {hero_id}; choices={choices:?}");
        }
        let max_rank = Destiny::rank_limit(db, stone);
        if rank <= 0 || rank > max_rank {
            bail!("destiny rank {rank} is invalid for stone {stone}; valid=1..={max_rank}");
        }
        Destiny::exchanges(db, stone, rank)
    } else {
        None
    };

    if let Some((psychube_id, level)) = psychube {
        if db.equip.get(psychube_id).is_none() {
            bail!("psychube {psychube_id} is missing from equip config");
        }
        if !db
            .equip_skill
            .iter()
            .any(|row| row.id == psychube_id && row.skill_lv == level)
        {
            let levels = db
                .equip_skill
                .iter()
                .filter(|row| row.id == psychube_id)
                .map(|row| row.skill_lv)
                .collect::<Vec<_>>();
            bail!("psychube level {level} is invalid for {psychube_id}; choices={levels:?}");
        }
    }

    let destiny_stone = destiny_selection
        .map(|(stone, _)| stone)
        .unwrap_or_default();
    // Device ownership and device skills are chosen per Portrait level.
    let levels = std::iter::once(0)
        .chain(if destiny_stone > 0 {
            db.destiny_facets_ex_level
                .iter()
                .filter(|row| row.hero_id == destiny_stone)
                .map(|row| row.skill_level)
                .collect::<Vec<_>>()
        } else {
            db.skill_ex_level
                .iter()
                .filter(|row| row.hero_id == hero_id)
                .map(|row| row.skill_level)
                .collect()
        })
        .collect::<Vec<_>>();
    let mut character_kit_reachable = false;
    for level in levels {
        match battle::catalog::configured_conduit_skill_ids(db, hero_id, level, destiny_stone)
            .map_err(|error| anyhow::anyhow!("resolve configured device skills: {error:?}"))?
        {
            Some(device_skills) => {
                for skill_id in device_skills {
                    enqueue(
                        skills,
                        skill_id,
                        format!("hero {hero_id} > device Portrait {level}"),
                    );
                }
            }
            None => character_kit_reachable = true,
        }
    }
    if character_kit_reachable {
        // Scan every kit tier a player can own, not only the max chain.
        let base = [("base", hero.skill.as_str(), hero.ex_skill)];
        let replaced = db
            .character_rank_replace
            .get(hero_id)
            .map(|row| ("Insight replacement", row.skill.as_str(), row.ex_skill));
        for (tier, skill, ex_skill) in base.into_iter().chain(replaced) {
            enqueue_kit_tier(
                skills,
                &format!("hero {hero_id} > {tier}"),
                [parse_skill_group(skill, 1), parse_skill_group(skill, 2)],
                ex_skill,
                destiny.as_ref(),
            );
        }
        for gear in db.gear_rows(hero_id) {
            let path = format!(
                "hero {hero_id} > gear {}#{} Portrait {}",
                gear.first_id, gear.second_id, gear.skill_level
            );
            enqueue_kit_tier(
                skills,
                &path,
                [
                    configured_skill_ids(&gear.skill_group1, db),
                    configured_skill_ids(&gear.skill_group2, db),
                ],
                gear.skill_ex,
                destiny.as_ref(),
            );
            for skill_id in configured_skill_ids(&gear.passive_skill, db) {
                enqueue(skills, skill_id, format!("{path} passive"));
            }
            if !gear.exchange_skills.trim().is_empty() {
                report.warning(format!(
                    "GearExchangeSkillsUnsupported path={path} raw={:?}",
                    gear.exchange_skills
                ));
            }
        }
        for talent in db.talent_tree_rows(hero_id) {
            for level in 0..=5 {
                let (added, exchanges) = config::GameDB::talent_skills_at(talent, level);
                let path = format!(
                    "hero {hero_id} > talent {} Portrait {level}",
                    talent.talent_id
                );
                let targets = exchanges
                    .split('|')
                    .filter_map(|pair| pair.split_once('#').map(|(_, to)| to));
                for skill_id in configured_skill_ids(added, db)
                    .into_iter()
                    .chain(targets.flat_map(|to| configured_skill_ids(to, db)))
                {
                    enqueue(skills, skill_id, path.clone());
                }
            }
        }
        for row in db
            .skill_ex_level
            .iter()
            .filter(|row| row.hero_id == hero_id)
        {
            enqueue_kit_tier(
                skills,
                &format!("hero {hero_id} > Portrait {}", row.skill_level),
                [
                    configured_skill_family_ids(&row.skill_group1, db),
                    configured_skill_family_ids(&row.skill_group2, db),
                ],
                row.skill_ex,
                destiny.as_ref(),
            );
        }
    }
    for passive in Passive::configured(db, hero_id, psychube, destiny_selection) {
        enqueue(
            skills,
            passive.skill_id,
            format!(
                "hero {hero_id} > {:?} {} rank {}",
                passive.source.kind, passive.source.source_id, passive.source.rank
            ),
        );
    }
    if destiny_selection.is_none() && !Destiny::stones(db, hero_id).is_empty() {
        report.warning(format!(
            "DestinyStoneNotSelected path=hero {hero_id} choices={:?}",
            Destiny::stones(db, hero_id)
        ));
    }
    Ok(())
}

fn enqueue_kit_tier(
    skills: &mut VecDeque<Pending>,
    path: &str,
    groups: [Vec<i32>; 2],
    ex_skill: i32,
    destiny: Option<&std::collections::HashMap<i32, i32>>,
) {
    for (index, mut group) in groups.into_iter().enumerate() {
        apply_destiny(&mut group, destiny);
        for skill_id in group {
            enqueue(
                skills,
                skill_id,
                format!("{path} skill group {}", index + 1),
            );
        }
    }
    let ex_skill = destiny
        .and_then(|map| map.get(&ex_skill).copied())
        .unwrap_or(ex_skill);
    if ex_skill > 0 {
        enqueue(skills, ex_skill, format!("{path} ultimate"));
    }
}

pub(crate) fn collect_episode_roots(
    episode_id: i32,
    db: &config::GameDB,
    skills: &mut VecDeque<Pending>,
    report: &mut Report,
) -> Result<()> {
    let episode = db
        .episode
        .get(episode_id)
        .with_context(|| format!("episode {episode_id} is missing"))?;
    collect_battle_roots(episode_id, episode.battle_id, db, skills, report)
}

pub(crate) fn collect_battle_roots(
    episode_id: i32,
    battle_id: i32,
    db: &config::GameDB,
    skills: &mut VecDeque<Pending>,
    report: &mut Report,
) -> Result<()> {
    let battle = db
        .battle
        .get(battle_id)
        .with_context(|| format!("battle {battle_id} is missing"))?;
    for group_id in split_ids(&battle.monster_group_ids) {
        let Some(group) = db.monster_group.get(group_id) else {
            report.error(format!(
                "MissingMonsterGroup path=episode {episode_id} > battle {} group={group_id}",
                battle.id
            ));
            continue;
        };
        for monster_id in split_ids(&group.monster) {
            let path = format!(
                "episode {episode_id} > battle {battle_id} > group {group_id} > monster {monster_id}"
            );
            enqueue_monster_skills(db, monster_id, &path, skills, report);
        }
    }
    for rule_id in split_ids(&battle.addition_rule)
        .into_iter()
        .chain(split_ids(&battle.hidden_rule))
        .filter(|rule_id| db.rule.get(*rule_id).is_some())
    {
        let rule = db.rule.get(rule_id).unwrap();
        for skill_id in configured_skill_ids(&rule.effect, db) {
            enqueue(
                skills,
                skill_id,
                format!("episode {episode_id} > battle {battle_id} > rule {rule_id}"),
            );
        }
    }
    Ok(())
}

pub(crate) fn collect_tower_assist_boss_roots(
    tower_id: i32,
    db: &config::GameDB,
    skills: &mut VecDeque<Pending>,
) -> Result<()> {
    let boss = db
        .tower_assist_boss
        .iter()
        .find(|boss| boss.tower_id == tower_id)
        .with_context(|| format!("tower {tower_id} is missing its assist boss config"))?;
    let path = format!("tower {tower_id} > assist boss {}", boss.boss_id);
    for skill_id in configured_skill_ids(&boss.active_skills, db)
        .into_iter()
        .chain(configured_skill_ids(&boss.passive_skills, db))
        .chain(configured_skill_ids(&boss.teach_skills, db))
    {
        enqueue(skills, skill_id, path.clone());
    }
    for form in db
        .tower_assist_boss_change
        .iter()
        .filter(|form| form.boss_id == boss.boss_id)
    {
        let form_path = format!("{path} > form {}", form.form);
        for skill_id in configured_skill_ids(&form.active_skills, db)
            .into_iter()
            .chain(configured_skill_ids(&form.passive_skills, db))
            .chain(configured_skill_ids(&form.replace_passive_skills, db))
        {
            enqueue(skills, skill_id, form_path.clone());
        }
    }
    Ok(())
}
