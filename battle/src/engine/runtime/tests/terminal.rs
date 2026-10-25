use super::*;
use crate::engine::skill::{
    behavior::classify::BehaviorSpec,
    condition::{ParsedCondition, ParsedConditionKind, none::NoneMode},
    effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
    rule::route::ConditionRoute,
    target::TargetRequest,
};

mod settlement_and_rounds;
mod waves;
