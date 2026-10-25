//! Damage-handler regression groups.

use super::*;
use crate::engine::{
    damage::{
        AttackPlan, DamageFormula, DamageFormulaInput as DamageInputs, DamageKind, DamageRateTerm,
        MinimumDamage,
    },
    manager::BattleManagers,
    skill::{
        behavior::classify::BehaviorSpec,
        rule::CommandOrigin,
        target::{TargetContext, TargetPool},
    },
};
use crate::test_support::init_config;
use sonettobuf::{
    BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute, HeroSpAttribute,
};

fn entity(uid: i64, team_type: i32, career: i32, attack: i32, defense: i32) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        career: Some(career),
        current_hp: Some(1000),
        attr: Some(HeroAttribute {
            hp: Some(1000),
            attack: Some(attack),
            defense: Some(defense),
            mdefense: Some(defense),
            ..Default::default()
        }),
        ..Default::default()
    }
}

mod additional;
mod arithmetic;
mod critical;
mod fixed_and_butterfly;
mod formula;
mod lost_life;
mod origin_rules;
mod skill_might;
