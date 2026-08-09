use sonettobuf::{BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, PowerInfo};

use super::*;
use crate::engine::{
    manager::{
        card::CardPlay,
        field::{FieldCommand, FieldDefinition, FieldOperation},
    },
    mechanic::impromptu::{build_plan, enable_rule_ops, inspiration_key, team_energy_key},
    skill::{
        behavior::classify::BehaviorSpec,
        buff_act,
        condition::{
            ParsedCondition, ParsedConditionKind, buff::BuffConditionMode, none::NoneMode,
        },
        effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        rule::{CommandOrigin, DefinitionKey, RuleDomain, route::ConditionRoute},
        target::TargetRequest,
    },
};
use crate::test_support::init_config;

mod actions;
mod entry;
mod mechanics;
mod refill;
mod round_start;
mod settlement;
mod start;
