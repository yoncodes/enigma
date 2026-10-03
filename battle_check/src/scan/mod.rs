use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use battle::tooling::scan::{
    BuffPolicy, ConditionDriver, ConditionRoute, ConditionTiming, Destiny, ParsedBehavior,
    ParsedCondition, ParsedConditionKind, Passive, RouteError, SkillEffectCatalog, SkillEffectSlot,
    behavior::{self, is_supported},
    buff_act::{
        self,
        effect_time::{BuffActEvent, classify as classify_effect_time},
        registry as buff_act_registry,
    },
    condition_registry as registry, halo,
    hero_skill::{parse_skill_group, split_ids},
    is_mapped_target_code,
};

use crate::options::Options;

mod closure;
mod report;
mod roots;

pub(crate) use closure::scan_closure;
#[cfg(test)]
use closure::{buff_act_capability, enqueue_summoned_skills, malformed_buff_act_error};
pub(crate) use report::{CapabilityKey, Report};
pub(crate) use roots::{
    Pending, collect_battle_roots, collect_episode_roots, collect_hero_build_roots,
    collect_hero_roots, collect_tower_assist_boss_roots,
};

#[cfg(test)]
mod tests;
