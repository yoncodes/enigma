use super::*;

impl HeroManager {
    pub async fn choice_weapon(
        self,
        db: &SqlitePool,
        hero_id: i32,
        main_id: i32,
        sub_id: i32,
    ) -> Result<(ChoiceHero3123WeaponReply, HeroInfo), AppError> {
        let game = config::configs::get();
        if game.gear_rows(hero_id).next().is_none() {
            return Err(AppError::InvalidRequest);
        }
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let data = hero.get_hero(hero_id).await?;
        let unlocked = |second| {
            game.gear_slot_unlock_rank(second)
                .is_some_and(|rank| data.record.rank >= rank)
        };
        if (main_id != 0 && !unlocked(false)) || (sub_id != 0 && !unlocked(true)) {
            return Err(AppError::InvalidRequest);
        }
        if (main_id != 0 || sub_id != 0)
            && !game.has_character_weapon(hero_id, main_id, sub_id, data.record.ex_skill_level)
        {
            return Err(AppError::InvalidRequest);
        }
        hero.update_special_equipped_gear(hero_id, format!("{main_id}#{sub_id}"))
            .await?;
        let updated = snapshot(db, hero.get_hero(hero_id).await?).await?;

        Ok((
            ChoiceHero3123WeaponReply {
                hero_id: Some(hero_id),
                main_id: Some(main_id),
                sub_id: Some(sub_id),
            },
            updated,
        ))
    }

    pub async fn choose_talent(
        self,
        db: &SqlitePool,
        hero_id: i32,
        sub_id: i32,
        level: i32,
    ) -> Result<(ChoiceHero3124TalentTreeReply, HeroInfo), AppError> {
        let extra_str = self
            .update_talent_tree(db, hero_id, sub_id, level, true)
            .await?;
        let updated = snapshot(
            db,
            UserHeroModel::new(self.player_id, db.clone())
                .get_hero(hero_id)
                .await?,
        )
        .await?;

        Ok((
            ChoiceHero3124TalentTreeReply {
                hero_id: Some(hero_id),
                extra_str: Some(extra_str),
            },
            updated,
        ))
    }

    pub async fn cancel_talent(
        self,
        db: &SqlitePool,
        hero_id: i32,
        sub_id: i32,
        level: i32,
    ) -> Result<(CancelHero3124TalentTreeReply, HeroInfo), AppError> {
        let extra_str = self
            .update_talent_tree(db, hero_id, sub_id, level, false)
            .await?;
        let updated = snapshot(
            db,
            UserHeroModel::new(self.player_id, db.clone())
                .get_hero(hero_id)
                .await?,
        )
        .await?;

        Ok((
            CancelHero3124TalentTreeReply {
                hero_id: Some(hero_id),
                extra_str: Some(extra_str),
            },
            updated,
        ))
    }

    pub async fn reset_talents(
        self,
        db: &SqlitePool,
        hero_id: i32,
    ) -> Result<(ResetHero3124TalentTreeReply, HeroInfo), AppError> {
        if !owns_talent_tree(hero_id) {
            return Err(AppError::InvalidRequest);
        }
        let hero = UserHeroModel::new(self.player_id, db.clone());
        hero.update_special_equipped_gear(hero_id, String::new())
            .await?;
        let updated = snapshot(db, hero.get_hero(hero_id).await?).await?;

        Ok((
            ResetHero3124TalentTreeReply {
                hero_id: Some(hero_id),
                extra_str: Some(String::new()),
            },
            updated,
        ))
    }

    async fn update_talent_tree(
        self,
        db: &SqlitePool,
        hero_id: i32,
        sub_id: i32,
        level: i32,
        add: bool,
    ) -> Result<String, AppError> {
        if !owns_talent_tree(hero_id) || hero_3124_talent_id(sub_id, level).is_none() {
            return Err(AppError::InvalidRequest);
        }
        let hero = UserHeroModel::new(self.player_id, db.clone());
        let data = hero.get_hero(hero_id).await?;
        let extra_str = if add {
            let points = config::configs::get().talent_points(data.record.rank);
            light_talents(&data.record.extra_str, sub_id, level, points)
                .ok_or(AppError::InvalidRequest)?
        } else {
            cancel_talents(&data.record.extra_str, sub_id, level).ok_or(AppError::InvalidRequest)?
        };

        hero.update_special_equipped_gear(hero_id, extra_str.clone())
            .await?;

        Ok(extra_str)
    }
}

fn owns_talent_tree(hero_id: i32) -> bool {
    config::configs::get()
        .talent_tree_rows(hero_id)
        .next()
        .is_some()
}

pub(super) fn hero_3124_talent_id(sub_id: i32, level: i32) -> Option<i32> {
    config::configs::get()
        .hero_skill_talent(sub_id, level)
        .map(|talent| talent.talent_id)
}

fn hero_3124_talent_level(sub_id: i32, talent_id: i32) -> Option<i32> {
    config::configs::get().hero_skill_talent_level(sub_id, talent_id)
}

/// Lights every level up to `level` in the branch, like the client. A new
/// branch can start only once three talents are lit in total, and lit talents
/// cannot exceed the rank's points.
pub(super) fn light_talents(
    extra_str: &str,
    sub_id: i32,
    level: i32,
    points: i32,
) -> Option<String> {
    const TREE_NODES: usize = 3;
    let mut talents = parse_talent_extra_str(extra_str);
    let total = talents.values().map(BTreeSet::len).sum::<usize>();
    if !talents.contains_key(&sub_id) && total > 0 && total < TREE_NODES {
        return None;
    }
    let ids = (1..=level)
        .map(|level| hero_3124_talent_id(sub_id, level))
        .collect::<Option<Vec<_>>>()?;
    talents.entry(sub_id).or_default().extend(ids);
    let lit = talents.values().map(BTreeSet::len).sum::<usize>();
    (lit <= usize::try_from(points).unwrap_or_default()).then(|| format_talent_extra_str(&talents))
}

/// Cancels `level` and every higher level in the branch. Like the client, a
/// full branch is locked while another branch has talents.
pub(super) fn cancel_talents(extra_str: &str, sub_id: i32, level: i32) -> Option<String> {
    const TREE_NODES: usize = 3;
    let mut talents = parse_talent_extra_str(extra_str);
    if talents.len() > 1
        && talents
            .get(&sub_id)
            .is_some_and(|ids| ids.len() >= TREE_NODES)
    {
        return None;
    }
    if let Some(sub_talents) = talents.get_mut(&sub_id) {
        sub_talents.retain(|id| {
            hero_3124_talent_level(sub_id, *id).is_none_or(|talent_level| talent_level < level)
        });
    }
    Some(format_talent_extra_str(&talents))
}

fn parse_talent_extra_str(extra_str: &str) -> BTreeMap<i32, BTreeSet<i32>> {
    let mut talents = BTreeMap::new();
    for group in extra_str.split('|').filter(|group| !group.is_empty()) {
        let Some((sub_id, ids)) = group.split_once('#') else {
            continue;
        };
        let Ok(sub_id) = sub_id.parse::<i32>() else {
            continue;
        };

        talents.insert(
            sub_id,
            ids.split(',')
                .filter_map(|id| id.parse::<i32>().ok())
                .collect(),
        );
    }

    talents
}

fn format_talent_extra_str(talents: &BTreeMap<i32, BTreeSet<i32>>) -> String {
    talents
        .iter()
        .filter(|(_, ids)| !ids.is_empty())
        .map(|(sub_id, ids)| {
            let ids = ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
            format!("{sub_id}#{ids}")
        })
        .collect::<Vec<_>>()
        .join("|")
}
