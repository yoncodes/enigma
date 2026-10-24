use super::*;
use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffEntryHaloFanout},
        card::{CardCommand, CardSetUltimateAvailability},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
};

mod dispatch;
mod event;
mod queue;

use dispatch::terminal_owner_scope;
pub(super) use dispatch::{dispatch_owner_reactions, dispatch_reactions};
#[cfg(test)]
pub(super) use event::dispatch_event_batch;
pub(super) use event::{dispatch_event_groups, ordered_hit_entities};
pub(super) use queue::{queued_reactions, reaction_counterparty, reaction_skill_target};
