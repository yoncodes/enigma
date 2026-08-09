use super::closure::{apply_destiny, configured_skill_ids, enqueue, enqueue_monster_skills};
use super::*;

#[derive(Debug)]
pub(crate) struct Pending {
    pub(crate) id: i32,
    pub(super) path: String,
}

pub(crate) fn collect_hero_roots(
    options: &Options,
    hero_id: i32,
    db: &config::GameDB,
    skills: &mut VecDeque<Pending>,
    report: &mut Report,
) -> Result<()> {
    let hero = db
        .character
        .get(hero_id)
        .with_context(|| format!("hero {hero_id} is missing from character config"))?;

    let destiny = if let Some(stone) = options.destiny_stone {
        let choices = Destiny::stones(db, hero_id);
        if !choices.contains(&stone) {
            bail!("destiny stone {stone} is not available to hero {hero_id}; choices={choices:?}");
        }
        let rank = options.destiny_rank.unwrap();
        let max_rank = Destiny::rank_limit(db, stone);
        if rank <= 0 || rank > max_rank {
            bail!("destiny rank {rank} is invalid for stone {stone}; valid=1..={max_rank}");
        }
        println!("destiny_stone={stone} rank={rank}");
        Destiny::exchanges(db, stone, rank)
    } else {
        None
    };

    if let Some(psychube_id) = options.psychube_id {
        if db.equip.get(psychube_id).is_none() {
            bail!("psychube {psychube_id} is missing from equip config");
        }
        let level = options.psychube_level.unwrap();
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
        println!("psychube={psychube_id} skill_rank={level}");
    }

    let mut group1 = parse_skill_group(&hero.skill, 1);
    let mut group2 = parse_skill_group(&hero.skill, 2);
    let mut ex_skill = hero.ex_skill;
    let mut upgrades = db
        .skill_ex_level
        .iter()
        .filter(|row| row.hero_id == hero_id)
        .collect::<Vec<_>>();
    upgrades.sort_by_key(|row| row.skill_level);
    for row in upgrades {
        if !row.skill_group1.trim().is_empty() {
            group1 = configured_skill_ids(&row.skill_group1, db);
        }
        if !row.skill_group2.trim().is_empty() {
            group2 = configured_skill_ids(&row.skill_group2, db);
        }
        if row.skill_ex != 0 {
            ex_skill = row.skill_ex;
        }
    }
    apply_destiny(&mut group1, destiny.as_ref());
    apply_destiny(&mut group2, destiny.as_ref());
    ex_skill = destiny
        .as_ref()
        .and_then(|map| map.get(&ex_skill).copied())
        .unwrap_or(ex_skill);

    for (label, skill_ids) in [("skill group 1", group1), ("skill group 2", group2)] {
        for skill_id in skill_ids {
            enqueue(skills, skill_id, format!("hero {hero_id} > max {label}"));
        }
    }
    if ex_skill > 0 {
        enqueue(skills, ex_skill, format!("hero {hero_id} > ultimate"));
    }
    for passive in Passive::configured(
        db,
        hero_id,
        options.psychube_id.zip(options.psychube_level),
        options.destiny_stone.zip(options.destiny_rank),
    ) {
        enqueue(
            skills,
            passive.skill_id,
            format!(
                "hero {hero_id} > {:?} {} rank {}",
                passive.source.kind, passive.source.source_id, passive.source.rank
            ),
        );
    }
    if options.destiny_stone.is_none() && !Destiny::stones(db, hero_id).is_empty() {
        report.warning(format!(
            "DestinyStoneNotSelected path=hero {hero_id} choices={:?}",
            Destiny::stones(db, hero_id)
        ));
    }
    Ok(())
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
