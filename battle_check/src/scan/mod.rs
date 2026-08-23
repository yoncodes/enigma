use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use battle::engine::{
    buff::halo,
    entity::{
        destiny::Destiny,
        passive::Passive,
        skill::{parse_skill_group, split_ids},
    },
    manager::buff::BuffPolicy,
    skill::{
        behavior::{self, is_supported},
        buff_act::{
            self,
            effect_time::{BuffActEvent, classify as classify_effect_time},
            registry as buff_act_registry,
        },
        condition::{ConditionTiming, ParsedCondition, ParsedConditionKind, registry},
        effect::{ParsedBehavior, SkillEffectCatalog, SkillEffectSlot},
        rule::route::{ConditionDriver, ConditionRoute, RouteError},
        target::is_mapped_target_code,
    },
};

use crate::options::Options;

mod closure;
mod report;
mod roots;

pub(crate) use closure::scan_closure;
#[cfg(test)]
use closure::{buff_act_capability, malformed_buff_act_error};
pub(crate) use report::{CapabilityKey, Report};
pub(crate) use roots::{
    Pending, collect_battle_roots, collect_episode_roots, collect_hero_build_roots,
    collect_hero_roots, collect_tower_assist_boss_roots,
};

#[cfg(test)]
mod tests;
