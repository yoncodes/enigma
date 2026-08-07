use std::collections::HashMap;

use config::configs;
use database::{
    db::game::equipment::{self, Equipment},
    models::game::heros::HeroData,
};
use sonettobuf::{HeroAttribute, HeroExAttribute, HeroSpAttribute};
use sqlx::SqlitePool;

use super::attr::AttrId;

#[derive(Debug, Clone, Default)]
pub struct StatInputs {
    pub hero_id: i32,
    pub level: i32,
    pub rank: i32,
    pub destiny_rank: i32,
    pub equip_id: i32,
    pub equip_level: i32,
    pub equip_break_level: i32,
    pub talent: i32,
    pub talent_style: i32,
    pub talent_placements: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleBalance {
    pub level: i32,
    pub talent: i32,
    pub equip_level: i32,
}

impl BattleBalance {
    pub fn parse(raw: &str) -> Option<Self> {
        let mut values = raw.split('#').map(str::parse::<i32>);
        let balance = Self {
            level: values.next()?.ok()?,
            talent: values.next()?.ok()?,
            equip_level: values.next()?.ok()?,
        };
        (values.next().is_none()
            && balance.level > 0
            && balance.talent > 0
            && balance.equip_level > 0)
            .then_some(balance)
    }

    pub fn apply(self, mut input: StatInputs) -> StatInputs {
        let max_level = configs::get()
            .character_level
            .iter()
            .filter(|row| row.hero_id == input.hero_id)
            .map(|row| row.level)
            .max()
            .unwrap_or(input.level);
        let level = self.level.min(max_level).max(input.level);
        let rank = rank_from_level(input.hero_id, level).max(input.rank);
        let talent = configs::get()
            .character_talent
            .iter()
            .filter(|row| {
                row.hero_id == input.hero_id
                    && row.talent_id <= self.talent
                    && row.requirement <= rank
            })
            .map(|row| row.talent_id)
            .max()
            .unwrap_or(1);
        if talent > input.talent || (input.rank < 3 && rank >= 3) {
            input.talent = talent;
            input.talent_style = 0;
            input.talent_placements.clear();
        }
        input.level = level;
        input.rank = rank;
        input.equip_level = input.equip_level.max(self.equip_level);
        input
    }

    pub fn stats_for(self, hero: &HeroData, equips: &[Equipment]) -> Stats {
        let base = self.apply(StatInputs::from_hero_data(hero, None));
        equips.iter().fold(Stats::build(&base), |stats, equip| {
            let input = self.apply(StatInputs::from_hero_data(hero, Some(equip)));
            stats + Stats::equipment_bonus(&input)
        })
    }
}

impl StatInputs {
    pub fn from_hero_data(hero: &HeroData, equip: Option<&Equipment>) -> Self {
        let record = &hero.record;
        let template = hero
            .talent_templates
            .iter()
            .find(|(template, _)| template.template_id == record.use_talent_template_id)
            .or_else(|| hero.talent_templates.first());
        let cubes = template
            .filter(|(_, cubes)| !cubes.is_empty())
            .map(|(_, cubes)| cubes.as_slice())
            .unwrap_or(&hero.talent_cubes);
        Self {
            hero_id: record.hero_id,
            level: record.level,
            rank: record.rank,
            destiny_rank: record.destiny_rank,
            equip_id: equip.map(|equip| equip.equip_id).unwrap_or_default(),
            equip_level: equip.map(|equip| equip.level).unwrap_or_default(),
            equip_break_level: equip.map(|equip| equip.break_lv).unwrap_or_default(),
            talent: record.talent,
            talent_style: template
                .map(|(template, _)| template.style)
                .unwrap_or_default(),
            talent_placements: cubes.iter().map(|cube| cube.cube_id).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub mdef: i32,
    pub technic: i32,
    pub cri: i32,
    pub recri: i32,
    pub cri_dmg: i32,
    pub cri_def: i32,
    pub add_dmg: i32,
    pub drop_dmg: i32,
    pub normal_skill_rate: i32,
    pub clutch: i32,
    pub revive: i32,
    pub absorb: i32,
    pub heal: i32,
    pub defense_ignore: i32,
    pub rebound_dmg: i32,
    pub extra_dmg: i32,
    pub reuse_dmg: i32,
    pub poison_add_rate: i32,
}

impl std::ops::Add for Stats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            hp: self.hp + rhs.hp,
            atk: self.atk + rhs.atk,
            def: self.def + rhs.def,
            mdef: self.mdef + rhs.mdef,
            technic: self.technic + rhs.technic,
            cri: self.cri + rhs.cri,
            recri: self.recri + rhs.recri,
            cri_dmg: self.cri_dmg + rhs.cri_dmg,
            cri_def: self.cri_def + rhs.cri_def,
            add_dmg: self.add_dmg + rhs.add_dmg,
            drop_dmg: self.drop_dmg + rhs.drop_dmg,
            normal_skill_rate: self.normal_skill_rate + rhs.normal_skill_rate,
            clutch: self.clutch + rhs.clutch,
            revive: self.revive + rhs.revive,
            absorb: self.absorb + rhs.absorb,
            heal: self.heal + rhs.heal,
            defense_ignore: self.defense_ignore + rhs.defense_ignore,
            rebound_dmg: self.rebound_dmg + rhs.rebound_dmg,
            extra_dmg: self.extra_dmg + rhs.extra_dmg,
            reuse_dmg: self.reuse_dmg + rhs.reuse_dmg,
            poison_add_rate: self.poison_add_rate + rhs.poison_add_rate,
        }
    }
}

impl Stats {
    pub fn build_for_hero(hero: &HeroData, equips: &[Equipment]) -> Self {
        let base = StatInputs::from_hero_data(hero, None);
        equips.iter().fold(Self::build(&base), |stats, equip| {
            let input = StatInputs::from_hero_data(hero, Some(equip));
            stats + Self::equipment_bonus(&input)
        })
    }

    pub fn build(input: &StatInputs) -> Self {
        let level = level_base(input.hero_id, input.level);
        let rank = rank_bonus(input.hero_id, input.rank);
        let core = apply_destiny(
            level,
            rank,
            destiny_bonus(input.hero_id, input.destiny_rank),
        );
        core + talent_bonus(input, level, rank) + Self::equipment_bonus(input)
    }

    pub(super) fn equipment_bonus(input: &StatInputs) -> Self {
        equip_bonus(input)
            + equip_break_bonus(
                input,
                level_base(input.hero_id, input.level) + rank_bonus(input.hero_id, input.rank),
            )
    }

    pub fn base(self) -> HeroAttribute {
        HeroAttribute {
            hp: Some(self.hp),
            attack: Some(self.atk),
            defense: Some(self.def),
            mdefense: Some(self.mdef),
            technic: Some(self.technic),
            multi_hp_idx: Some(0),
            multi_hp_num: Some(0),
        }
    }

    pub fn ex(self) -> HeroExAttribute {
        HeroExAttribute {
            cri: Some(self.cri),
            recri: Some(self.recri),
            cri_dmg: Some(self.cri_dmg),
            cri_def: Some(self.cri_def),
            add_dmg: Some(self.add_dmg),
            drop_dmg: Some(self.drop_dmg),
        }
    }

    pub fn sp(self) -> HeroSpAttribute {
        HeroSpAttribute {
            revive: Some(self.revive),
            heal: Some(self.heal),
            absorb: Some(self.absorb),
            defense_ignore: Some(self.defense_ignore),
            clutch: Some(self.clutch),
            normal_skill_rate: Some(self.normal_skill_rate),
            rebound_dmg: Some(self.rebound_dmg),
            extra_dmg: Some(self.extra_dmg),
            reuse_dmg: Some(self.reuse_dmg),
            ..Default::default()
        }
    }
}

pub async fn hero_info(pool: &SqlitePool, hero: HeroData) -> anyhow::Result<sonettobuf::HeroInfo> {
    let equip = if hero.record.default_equip_uid == 0 {
        None
    } else {
        Some(
            equipment::get_equipment_by_uid(
                pool,
                hero.record.user_id,
                hero.record.default_equip_uid,
            )
            .await?,
        )
    };
    let stats = Stats::build(&StatInputs::from_hero_data(&hero, equip.as_ref()));
    Ok(hero.into_proto(stats.base(), stats.ex(), stats.sp()))
}

pub fn monster_instance_ex_stats(model_id: i32, level: i32) -> Option<Stats> {
    let game = configs::get();
    let monster = game.monster.get(model_id)?;
    let skill = game.monster_skill_template.get(monster.skill_template)?;
    let instance = game.monster_instance.get(skill.instance)?;
    let sub = game.monster_sub.get(instance.sub)?;
    let job = game.monster_job.get(sub.job)?;
    let level = if level > 0 {
        level
    } else {
        monster.level_true.max(monster.level)
    };
    let curve = game.monster_level.get(level)?;
    let scaled = |initial: i32, equip: i32, resonance: i32| {
        ((i64::from(initial) * 10_000
            + i64::from(equip) * i64::from(curve.equip_super)
            + i64::from(resonance) * i64::from(curve.base_super))
            / 100_000) as i32
    };
    Some(Stats {
        cri: scaled(job.cri_init_super, job.cri_equip_super, job.cri_reson_super),
        recri: scaled(
            job.recri_init_super,
            job.recri_equip_super,
            job.recri_reson_super,
        ),
        cri_dmg: scaled(
            job.cri_dmg_init_super,
            job.cri_dmg_equip_super,
            job.cri_dmg_reson_super,
        ),
        cri_def: scaled(
            job.cri_def_init_super,
            job.cri_def_equip_super,
            job.cri_def_reson_super,
        ),
        add_dmg: scaled(
            job.add_dmg_init_super,
            job.add_dmg_equip_super,
            job.add_dmg_reson_super,
        ),
        drop_dmg: scaled(
            job.drop_dmg_init_super,
            job.drop_dmg_equip_super,
            job.drop_dmg_reson_super,
        ),
        ..Default::default()
    })
}

pub fn monster_stats(model_id: i32, level: i32) -> Option<Stats> {
    let game = configs::get();
    let monster = game.monster.get(model_id)?;
    let skill = game.monster_skill_template.get(monster.skill_template)?;
    let level = if level > 0 {
        level
    } else {
        monster.level_true.max(monster.level)
    };

    if skill.instance > 0 {
        let instance = game.monster_instance.get(skill.instance)?;
        let sub = game.monster_sub.get(instance.sub)?;
        let job = game.monster_job.get(sub.job)?;
        let stat_level = if monster.level_true > 0 {
            monster.level_true
        } else {
            instance.level
        };
        let curve = game.monster_level.get(stat_level)?;
        let scale = |sub: i32, job_base: i32, job_equip: i32, instance: i32, base: i32| {
            let core = i64::from(job_base) * i64::from(base)
                + i64::from(job_equip) * i64::from(curve.equip_base);
            (i64::from(sub) * core * i64::from(instance) / 1_000_000_000_000) as i32
        };
        let mut hp = scale(
            sub.life,
            job.life_base,
            job.life_equip_base,
            instance.life,
            curve.base,
        );
        if instance.multi_hp > 1 {
            hp /= instance.multi_hp;
        }
        let hidden = monster_instance_ex_stats(model_id, level).unwrap_or_default();
        return Some(Stats {
            hp,
            atk: scale(
                sub.attack,
                job.attack_base,
                job.attack_equip_base,
                instance.attack,
                curve.base,
            ),
            def: scale(
                sub.defense,
                job.defense_base,
                job.defense_equip_base,
                instance.defense,
                curve.base,
            ),
            mdef: scale(
                sub.mdefense,
                job.mdefense_base,
                job.mdefense_equip_base,
                instance.mdefense,
                curve.base,
            ),
            technic: scale(
                sub.technic,
                job.technic_base,
                job.technic_equip_base,
                instance.technic,
                curve.technic,
            ),
            ..hidden
        });
    }

    let template_id = if monster.template != 0 {
        monster.template
    } else {
        model_id
    };
    let template = game
        .monster_template
        .iter()
        .find(|template| template.template == template_id)?;
    Some(Stats {
        hp: template.life + template.life_grow * level,
        atk: template.attack + template.attack_grow * level,
        def: template.defense + template.defense_grow * level,
        mdef: template.mdefense + template.mdefense_grow * level,
        technic: template.technic + template.technic_grow * level,
        cri: template.cri + template.cri_grow * level,
        recri: template.recri + template.recri_grow * level,
        cri_dmg: template.cri_dmg + template.cri_dmg_grow * level,
        cri_def: template.cri_def + template.cri_def_grow * level,
        add_dmg: template.add_dmg + template.add_dmg_grow * level,
        drop_dmg: template.drop_dmg + template.drop_dmg_grow * level,
        ..Default::default()
    })
}

pub fn rank_from_level(hero_id: i32, level: i32) -> i32 {
    let mut rows = configs::get()
        .character_rank
        .iter()
        .filter(|row| row.hero_id == hero_id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.rank);
    rows.into_iter()
        .find(|row| {
            row.effect.split('|').any(|token| {
                let Some((kind, value)) = token.split_once('#') else {
                    return false;
                };
                kind == "1"
                    && value
                        .parse::<i32>()
                        .is_ok_and(|max_level| level <= max_level)
            })
        })
        .map(|row| row.rank)
        .unwrap_or(1)
}

fn level_base(hero_id: i32, level: i32) -> Stats {
    let game = configs::get();
    let mut rows = game
        .character_level
        .iter()
        .filter(|row| row.hero_id == hero_id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.level);
    let Some(lo) = rows.iter().rev().find(|row| row.level <= level).copied() else {
        return Stats::default();
    };
    let from = |row: &config::character_level::CharacterLevel| Stats {
        hp: row.hp,
        atk: row.atk,
        def: row.def,
        mdef: row.mdef,
        technic: row.technic,
        cri: row.cri,
        recri: row.recri,
        cri_dmg: row.cri_dmg,
        cri_def: row.cri_def,
        add_dmg: row.add_dmg,
        drop_dmg: row.drop_dmg,
        ..Default::default()
    };
    if lo.level == level {
        return from(lo);
    }
    let Some(hi) = rows.iter().find(|row| row.level > level).copied() else {
        return from(lo);
    };
    let lerp = |min: i32, max: i32| {
        min + (((max - min) as i64 * (level - lo.level) as i64) / (hi.level - lo.level) as i64)
            as i32
    };
    Stats {
        hp: lerp(lo.hp, hi.hp),
        atk: lerp(lo.atk, hi.atk),
        def: lerp(lo.def, hi.def),
        mdef: lerp(lo.mdef, hi.mdef),
        technic: lerp(lo.technic, hi.technic),
        ..from(lo)
    }
}

fn rank_bonus(hero_id: i32, rank: i32) -> Stats {
    if rank <= 0 {
        return Stats::default();
    }
    let base = level_base(hero_id, 1);
    configs::get()
        .character_rank
        .iter()
        .filter(|row| row.hero_id == hero_id && row.rank <= rank)
        .flat_map(|row| row.effect.split('|'))
        .filter_map(|token| token.split_once('#'))
        .filter_map(|(kind, value)| {
            (kind.trim() == "4")
                .then(|| value.trim().parse::<i32>().ok())
                .flatten()
        })
        .fold(Stats::default(), |sum, rate| sum + scale_base(base, rate))
}

fn scale_base(stats: Stats, rate: i32) -> Stats {
    let scale = |value| ((value as i64 * rate as i64) / 1000) as i32;
    Stats {
        hp: scale(stats.hp),
        atk: scale(stats.atk),
        def: scale(stats.def),
        mdef: scale(stats.mdef),
        technic: scale(stats.technic),
        ..Default::default()
    }
}

#[derive(Default)]
struct DestinyStats {
    flat: Stats,
    percent: Stats,
}

fn destiny_bonus(hero_id: i32, rank: i32) -> DestinyStats {
    let game = configs::get();
    let Some(destiny) = game
        .character_destiny
        .iter()
        .find(|row| row.hero_id == hero_id)
    else {
        return DestinyStats::default();
    };
    let mut result = DestinyStats::default();
    for token in game
        .character_destiny_slots
        .iter()
        .filter(|row| row.slots_id == destiny.slots_id && row.stage <= rank)
        .flat_map(|row| row.effect.split('|'))
    {
        let Some((id, value)) = token.split_once('#') else {
            continue;
        };
        let (Ok(id), Ok(value)) = (id.parse::<i32>(), value.parse::<i32>()) else {
            continue;
        };
        let target = match id {
            605..=608 => &mut result.percent,
            _ => &mut result.flat,
        };
        add_attr(target, id, value);
    }
    result
}

pub(crate) fn destiny_poison_add_rate(hero_id: i32, rank: i32) -> i32 {
    let Some(game) = configs::try_get() else {
        return 0;
    };
    let Some(destiny) = game
        .character_destiny
        .iter()
        .find(|row| row.hero_id == hero_id)
    else {
        return 0;
    };
    game.character_destiny_slots
        .iter()
        .filter(|row| row.slots_id == destiny.slots_id && row.stage <= rank)
        .flat_map(|row| row.effect.split('|'))
        .filter_map(parse_pair)
        .filter_map(|(id, value)| (id == AttrId::PoisonDmgBonus as i32).then_some(value))
        .sum()
}

fn apply_destiny(level: Stats, rank: Stats, destiny: DestinyStats) -> Stats {
    let scale = |base: i32, rank: i32, rate: i32| {
        (((base + rank) as i64 * (1000 + rate) as i64) / 1000) as i32
    };
    Stats {
        hp: scale(level.hp, rank.hp, destiny.percent.hp) + destiny.flat.hp,
        atk: scale(level.atk, rank.atk, destiny.percent.atk) + destiny.flat.atk,
        def: scale(level.def, rank.def, destiny.percent.def) + destiny.flat.def,
        mdef: scale(level.mdef, rank.mdef, destiny.percent.mdef) + destiny.flat.mdef,
        technic: level.technic + rank.technic,
        cri: level.cri + destiny.flat.cri,
        recri: level.recri + destiny.flat.recri,
        cri_dmg: level.cri_dmg + destiny.flat.cri_dmg,
        cri_def: level.cri_def + destiny.flat.cri_def,
        add_dmg: level.add_dmg + destiny.flat.add_dmg,
        drop_dmg: level.drop_dmg + destiny.flat.drop_dmg,
        ..destiny.flat
    }
}

fn add_attr(stats: &mut Stats, id: i32, value: i32) {
    match id {
        601 => stats.hp += value,
        602 => stats.atk += value,
        603 => stats.def += value,
        604 => stats.mdef += value,
        605 => stats.hp += value,
        606 => stats.atk += value,
        607 => stats.def += value,
        608 => stats.mdef += value,
        201 => stats.cri += value,
        202 => stats.recri += value,
        203 => stats.cri_dmg += value,
        204 => stats.cri_def += value,
        205 => stats.add_dmg += value,
        206 => stats.drop_dmg += value,
        209 => stats.revive += value,
        210 => stats.absorb += value,
        211 => stats.clutch += value,
        212 => stats.heal += value,
        213 => stats.defense_ignore += value,
        214 => stats.normal_skill_rate += value,
        218 => stats.rebound_dmg += value,
        219 => stats.extra_dmg += value,
        220 => stats.reuse_dmg += value,
        301 => stats.poison_add_rate += value,
        _ => {}
    }
}

fn equip_bonus(input: &StatInputs) -> Stats {
    let game = configs::get();
    let Some(equip) = game.equip.get(input.equip_id) else {
        return Stats::default();
    };
    let Some(row) = game
        .equip_strengthen
        .iter()
        .find(|row| row.strength_type == equip.strength_type)
    else {
        return Stats::default();
    };
    let rate = game
        .equip_strengthen_cost
        .iter()
        .find(|cost| cost.rare == equip.rare && cost.level == input.equip_level)
        .map(|cost| cost.attribute_rate)
        .unwrap_or(1000);
    let scale = |value| ((value as i64 * rate as i64) / 1000) as i32;
    Stats {
        hp: scale(row.hp),
        atk: scale(row.atk),
        def: scale(row.def),
        mdef: scale(row.mdef),
        ..Default::default()
    }
}

fn equip_break_bonus(input: &StatInputs, base: Stats) -> Stats {
    let game = configs::get();
    let Some(equip) = game.equip.get(input.equip_id) else {
        return Stats::default();
    };
    let inferred = game
        .equip_break_cost
        .iter()
        .filter(|row| row.rare == equip.rare && input.equip_level <= row.level)
        .map(|row| row.break_level)
        .min()
        .unwrap_or_default();
    let level = input.equip_break_level.max(inferred);
    let Some(row) = game
        .equip_break_attr
        .iter()
        .filter(|row| row.id == input.equip_id && row.break_level <= level)
        .max_by_key(|row| row.break_level)
    else {
        return Stats::default();
    };
    let scale = |value: i32, base: i32| ((value as i64 * base as i64) / 1000) as i32;
    Stats {
        hp: scale(row.hp, base.hp),
        atk: scale(row.attack, base.atk),
        def: scale(row.def, base.def),
        mdef: scale(row.mdef, base.mdef),
        cri: row.cri,
        recri: row.recri,
        cri_dmg: row.cri_dmg,
        cri_def: row.cri_def,
        add_dmg: row.add_dmg,
        drop_dmg: row.drop_dmg,
        normal_skill_rate: row.normal_skill_rate,
        clutch: row.clutch,
        revive: row.revive,
        absorb: row.absorb,
        heal: row.heal,
        defense_ignore: row.defense_ignore,
        ..Default::default()
    }
}

fn talent_bonus(input: &StatInputs, actual_base: Stats, actual_rank: Stats) -> Stats {
    let game = configs::get();
    let Some(character) = game
        .character_talent
        .iter()
        .find(|row| row.hero_id == input.hero_id && row.talent_id == input.talent)
    else {
        return Stats::default();
    };
    let Some((star_cube, star_level)) = parse_pair(&character.exclusive) else {
        return Stats::default();
    };
    let placements = if input.talent_placements.is_empty() {
        default_placements(input.talent, character.talent_mould, star_cube)
    } else {
        input.talent_placements.clone()
    };
    let style_cube = replace_cube(star_cube, input.talent_style);
    let counts = placements
        .into_iter()
        .map(|id| if id == star_cube { style_cube } else { id })
        .fold(HashMap::<i32, i32>::new(), |mut counts, id| {
            *counts.entry(id).or_default() += 1;
            counts
        });
    let mould = game
        .talent_mould
        .iter()
        .find(|row| row.talent_id == input.talent && row.talent_mould == character.talent_mould);
    let max_base = level_base(
        input.hero_id,
        game.character_level
            .iter()
            .filter(|row| row.hero_id == input.hero_id)
            .map(|row| row.level)
            .max()
            .unwrap_or(input.level),
    );
    let max_rank = rank_bonus(
        input.hero_id,
        game.character_rank
            .iter()
            .filter(|row| row.hero_id == input.hero_id)
            .map(|row| row.rank)
            .max()
            .unwrap_or(input.rank),
    );
    let damping = parse_damping();
    let mut total = [0.0_f64; 16];
    for (cube_id, count) in counts {
        let max_level = game
            .talent_cube_attr
            .iter()
            .filter(|row| row.id == cube_id)
            .map(|row| row.level)
            .max()
            .unwrap_or_default();
        let level = if cube_id == star_cube || cube_id == style_cube {
            star_level
        } else {
            cube_level(mould, cube_id)
        };
        let level = if level <= 0 {
            max_level
        } else {
            level.min(max_level)
        };
        let Some(row) = game
            .talent_cube_attr
            .iter()
            .find(|row| row.id == cube_id && row.level == level)
        else {
            continue;
        };
        let use_max = row.calculate_type == 1;
        let (base, rank) = if use_max {
            (max_base, max_rank)
        } else {
            (actual_base, actual_rank)
        };
        let damp = damping
            .iter()
            .filter(|(threshold, _)| count >= *threshold)
            .max_by_key(|(threshold, _)| *threshold)
            .map(|(_, rate)| *rate);
        for (index, value) in [row.hp, row.atk, row.def, row.mdef].into_iter().enumerate() {
            let bases = [
                base.hp + rank.hp,
                base.atk + rank.atk,
                base.def + rank.def,
                base.mdef + rank.mdef,
            ];
            let mut gain = bases[index] as f64 * value as f64 / 1000.0 * count as f64;
            if use_max {
                gain = gain.floor();
            }
            if let Some(rate) = damp {
                gain *= rate as f64 / 1000.0;
            }
            total[index] += gain;
        }
        for (index, value) in [
            row.cri,
            row.recri,
            row.cri_dmg,
            row.cri_def,
            row.add_dmg,
            row.drop_dmg,
            row.clutch,
            row.defense_ignore,
            row.normal_skill_rate,
            row.revive,
            row.absorb,
            row.heal,
        ]
        .into_iter()
        .enumerate()
        {
            let mut gain = value as f64 * count as f64;
            if let Some(rate) = damp {
                gain *= rate as f64 / 1000.0;
            }
            total[index + 4] += gain;
        }
    }
    Stats {
        hp: total[0].floor() as i32,
        atk: total[1].floor() as i32,
        def: total[2].floor() as i32,
        mdef: total[3].floor() as i32,
        cri: total[4].floor() as i32,
        recri: total[5].floor() as i32,
        cri_dmg: total[6].floor() as i32,
        cri_def: total[7].floor() as i32,
        add_dmg: total[8].floor() as i32,
        drop_dmg: total[9].floor() as i32,
        clutch: total[10].floor() as i32,
        defense_ignore: total[11].floor() as i32,
        normal_skill_rate: total[12].floor() as i32,
        revive: total[13].floor() as i32,
        absorb: total[14].floor() as i32,
        heal: total[15].floor() as i32,
        ..Default::default()
    }
}

fn parse_pair(raw: &str) -> Option<(i32, i32)> {
    let (left, right) = raw.split_once('#')?;
    Some((left.parse().ok()?, right.parse().ok()?))
}

fn default_placements(talent: i32, mould: i32, star_cube: i32) -> Vec<i32> {
    configs::get()
        .talent_scheme
        .iter()
        .find(|row| {
            row.talent_id == talent && row.talent_mould == mould && row.star_mould == star_cube
        })
        .map(|row| {
            row.talen_scheme
                .split('#')
                .filter_map(|token| token.split(',').next()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn replace_cube(star_cube: i32, style: i32) -> i32 {
    configs::get()
        .talent_style
        .iter()
        .filter(|row| row.style_id == style)
        .flat_map(|row| row.replace_cube.split('|'))
        .filter_map(parse_pair)
        .find_map(|(from, to)| (from == star_cube).then_some(to))
        .unwrap_or(star_cube)
}

fn cube_level(mould: Option<&config::talent_mould::TalentMould>, cube: i32) -> i32 {
    let Some(mould) = mould else { return 0 };
    let raw = match cube {
        10 => &mould.type10,
        11 => &mould.type11,
        12 => &mould.type12,
        13 => &mould.type13,
        14 => &mould.type14,
        15 => &mould.type15,
        16 => &mould.type16,
        17 => &mould.type17,
        18 => &mould.type18,
        19 => &mould.type19,
        20 => &mould.type20,
        _ => return 0,
    };
    parse_pair(raw).map(|(_, level)| level).unwrap_or_default()
}

fn parse_damping() -> Vec<(i32, i32)> {
    configs::get()
        .fight_const
        .get(10)
        .map(|row| row.value.as_str())
        .unwrap_or_default()
        .split('|')
        .filter_map(parse_pair)
        .collect()
}

#[cfg(test)]
mod tests;
