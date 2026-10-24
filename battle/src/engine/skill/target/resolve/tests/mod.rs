use crate::engine::{manager::BattleManagers, runtime::determinism::SkillTargetChoice};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::test_support::init_config;

fn resolve_code(
    code: i32,
    source_uid: i64,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
) -> Vec<i64> {
    TargetResolver::resolve(
        &TargetRequest {
            code,
            raw: Vec::new(),
        },
        1001,
        source_uid,
        pool,
        determinism,
    )
}

fn resolve_code_with_context(
    code: i32,
    source_uid: i64,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Vec<i64> {
    TargetResolver::resolve_with_context(
        &TargetRequest {
            code,
            raw: Vec::new(),
        },
        1001,
        source_uid,
        pool,
        determinism,
        context,
    )
}

fn entity_at(uid: i64, position: i32) -> FightEntityInfo {
    entity_stats(uid, position, 100, 100, 0)
}

fn enemy_with_buff(uid: i64, position: i32, current_hp: i32, buff: BuffInfo) -> FightEntityInfo {
    FightEntityInfo {
        buffs: vec![buff],
        ..entity_stats(uid, position, current_hp, 100, 0)
    }
}

fn buff(buff_id: i32, type_id: i32) -> BuffInfo {
    BuffInfo {
        buff_id: Some(buff_id),
        r#type: (type_id != 0).then_some(type_id),
        ..Default::default()
    }
}

fn queued_card(uid: i64, skill_id: i32) -> sonettobuf::CardInfo {
    sonettobuf::CardInfo {
        uid: Some(uid),
        skill_id: Some(skill_id),
        ..Default::default()
    }
}

fn entity_stats(
    uid: i64,
    position: i32,
    current_hp: i32,
    max_hp: i32,
    ex_point: i32,
) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        position: Some(position),
        current_hp: Some(current_hp),
        ex_point: Some(ex_point),
        attr: Some(HeroAttribute {
            hp: Some(max_hp),
            ..Default::default()
        }),
        ..Default::default()
    }
}

mod buffs;
mod deterministic;
mod exact;
mod groups;
mod mapping;
mod ordering;
mod redirect;
