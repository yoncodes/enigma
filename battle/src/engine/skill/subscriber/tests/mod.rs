use sonettobuf::{BuffInfo, EquipRecord, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::{
    condition::{ParsedCondition, ParsedConditionKind, buff::BuffConditionMode, none::NoneMode},
    effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
    target::{TargetPool, TargetRequest},
};

mod buff_team;
mod discovery;
mod ownership;
