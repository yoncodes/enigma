use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
fn grant_plan(plan: &BuffPlan) -> &GrantPlan {
    match &plan.action {
        BuffPlanAction::Grant(plan) => plan.as_ref(),
        BuffPlanAction::GrantInternalChild(plan) => plan.as_ref(),
        BuffPlanAction::Accumulate(plan) => plan.as_ref(),
        BuffPlanAction::Consume(_)
        | BuffPlanAction::ConsumeEffectCount(_)
        | BuffPlanAction::Convert(_)
        | BuffPlanAction::Replace(_)
        | BuffPlanAction::Remove(_)
        | BuffPlanAction::SetAmount(_)
        | BuffPlanAction::SetState(_)
        | BuffPlanAction::SetInternalState(_)
        | BuffPlanAction::SetStateSnapshot(_)
        | BuffPlanAction::AccumulateActValue(_)
        | BuffPlanAction::ChangeDuration(_)
        | BuffPlanAction::AddSpecialCount(_)
        | BuffPlanAction::ReserveChildUids(_)
        | BuffPlanAction::ReserveGrantUid(_)
        | BuffPlanAction::AdvanceDuration(_)
        | BuffPlanAction::SyncRoundStartDuration(_) => {
            panic!("expected grant plan")
        }
    }
}

mod consume;
mod fanout;
mod grant;
mod layers;
mod lifecycle;
mod uid;
