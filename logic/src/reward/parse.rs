use super::*;

pub fn parse(value: &str) -> RewardSet {
    let mut rewards = RewardSet::default();

    for part in value.split('|').filter(|part| !part.is_empty()) {
        let fields = part
            .split('#')
            .filter_map(|field| field.parse::<i32>().ok())
            .collect::<Vec<_>>();
        if fields.len() != 3 {
            continue;
        }

        match RewardMaterialType::from_i32(fields[0]) {
            Some(RewardMaterialType::Exp) => {
                rewards.player_exp = rewards.player_exp.saturating_add(fields[2]);
            }
            Some(RewardMaterialType::Item) => rewards.items.push((fields[1] as u32, fields[2])),
            Some(RewardMaterialType::Currency) => rewards.currencies.push((fields[1], fields[2])),
            Some(RewardMaterialType::BlockPackage) => {
                rewards.block_packages.push((fields[1], fields[2]));
            }
            Some(RewardMaterialType::Hero) => rewards.heroes.push((fields[1], fields[2])),
            Some(RewardMaterialType::HeroSkin) => rewards.skins.push((fields[1], fields[2])),
            Some(RewardMaterialType::PlayerCloth) => {
                rewards.player_cloths.push((fields[1], fields[2]));
            }
            Some(RewardMaterialType::PlayerClothExp) => {
                rewards.player_cloth_exp.push((fields[1], fields[2]));
            }
            Some(RewardMaterialType::Equip) => rewards.equips.push((fields[1], fields[2])),
            Some(RewardMaterialType::PowerPotion) => {
                rewards.power_items.push((fields[1], fields[2]));
            }
            Some(RewardMaterialType::Building) => {
                rewards.room_buildings.push((fields[1], fields[2]))
            }
            Some(RewardMaterialType::SpecialBlock) => {
                rewards.special_blocks.push((fields[1], fields[2]))
            }
            Some(RewardMaterialType::Antique) => rewards.antiques.push((fields[1], fields[2])),
            Some(RewardMaterialType::NewInsight) => {
                rewards.insight_items.push((fields[1], fields[2]));
            }
            Some(RewardMaterialType::Bp) => rewards.bp_scores.push((fields[1], fields[2])),
            Some(_) | None => {}
        }
    }

    rewards
}

pub fn parse_reward_id(reward_id: i32) -> RewardSet {
    parse_reward_id_with_cost(reward_id, 0)
}

fn parse_reward_id_with_cost(reward_id: i32, cost: i32) -> RewardSet {
    let Some(row) = config::configs::get().reward(reward_id) else {
        return RewardSet::default();
    };

    let mut rewards = RewardSet::default();
    for group in [
        &row.reward_group1,
        &row.reward_group2,
        &row.reward_group3,
        &row.reward_group4,
        &row.reward_group5,
        &row.reward_group6,
        &row.reward_group7,
        &row.reward_group8,
    ] {
        collect_reward_group(&mut rewards, group, cost);
    }

    rewards
}

pub fn parse_bonus(bonus_id: i32) -> RewardSet {
    parse_bonus_with_cost(bonus_id, 0)
}

pub fn parse_bonus_with_cost(bonus_id: i32, cost: i32) -> RewardSet {
    if bonus_id == 0 {
        return RewardSet::default();
    }

    let mut rewards = parse_reward_id_with_cost(bonus_id, cost);
    if let Some(bonus) = config::configs::get().bonus.get(bonus_id) {
        rewards.extend(parse(&bonus.fix_bonus));
        rewards.player_exp = rewards
            .player_exp
            .saturating_add(reward_count(&bonus.player_exp, cost).unwrap_or_default());
        if let Some(score) = reward_count(&bonus.score, cost).filter(|score| *score > 0) {
            rewards.currencies.push((3, score));
        }
    }
    rewards
}

pub fn hero_duplicate_rewards(hero_id: i32, duplicate_count: i32) -> Result<RewardSet, AppError> {
    let hero = config::configs::get()
        .character
        .get(hero_id)
        .ok_or(AppError::InvalidRequest)?;

    Ok(parse(if duplicate_count > 5 {
        &hero.duplicate_item2
    } else {
        &hero.duplicate_item
    }))
}

fn collect_reward_group(rewards: &mut RewardSet, group_ref: &str, cost: i32) {
    if group_ref.is_empty() {
        return;
    }

    let mut parts = group_ref.split(':');
    let Some(group) = parts.next().filter(|group| !group.is_empty()) else {
        return;
    };
    let mode = parts.next().unwrap_or("NORMAL");

    let rows = config::configs::get().reward_group(group);
    match mode {
        "NORMAL" => {
            for row in rows {
                collect_reward_group_row(rewards, row, cost);
            }
        }
        "WEIGHT" => {
            if let Some(row) = rows.into_iter().next() {
                collect_reward_group_row(rewards, row, cost);
            }
        }
        _ => {}
    }
}

fn collect_reward_group_row(
    rewards: &mut RewardSet,
    row: &config::reward_group::RewardGroup,
    cost: i32,
) {
    let Some(count) = reward_count(&row.count, cost) else {
        return;
    };
    if count <= 0 {
        return;
    }

    match RewardMaterialType::from_i32(row.material_type) {
        Some(RewardMaterialType::Exp) => {
            rewards.player_exp = rewards.player_exp.saturating_add(count);
        }
        Some(RewardMaterialType::Item) => rewards.items.push((row.material_id as u32, count)),
        Some(RewardMaterialType::Currency) => rewards.currencies.push((row.material_id, count)),
        Some(RewardMaterialType::BlockPackage) => {
            rewards.block_packages.push((row.material_id, count));
        }
        Some(RewardMaterialType::Hero) => rewards.heroes.push((row.material_id, count)),
        Some(RewardMaterialType::HeroSkin) => rewards.skins.push((row.material_id, count)),
        Some(RewardMaterialType::PlayerCloth) => {
            rewards.player_cloths.push((row.material_id, count));
        }
        Some(RewardMaterialType::PlayerClothExp) => {
            rewards.player_cloth_exp.push((row.material_id, count));
        }
        Some(RewardMaterialType::Equip) => rewards.equips.push((row.material_id, count)),
        Some(RewardMaterialType::PowerPotion) => rewards.power_items.push((row.material_id, count)),
        Some(RewardMaterialType::Building) => rewards.room_buildings.push((row.material_id, count)),
        Some(RewardMaterialType::SpecialBlock) => {
            rewards.special_blocks.push((row.material_id, count))
        }
        Some(RewardMaterialType::Antique) => rewards.antiques.push((row.material_id, count)),
        Some(RewardMaterialType::NewInsight) => {
            rewards.insight_items.push((row.material_id, count));
        }
        Some(RewardMaterialType::Bp) => rewards.bp_scores.push((row.material_id, count)),
        Some(_) | None => {}
    }
}

fn reward_count(value: &str, cost: i32) -> Option<i32> {
    value.parse().ok().or_else(|| {
        value
            .strip_suffix("*cost")
            .and_then(|factor| factor.parse::<i32>().ok())
            .map(|factor| factor.saturating_mul(cost))
    })
}
