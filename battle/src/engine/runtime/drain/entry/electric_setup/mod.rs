use crate::engine::skill::{
    behavior::classify::BehaviorKind, condition::ParsedConditionKind,
    effect::slot::SkillEffectSlot, subscriber::SetupSubscriber,
};

use super::*;

fn owns(slot: &SkillEffectSlot, catalog: crate::catalog::BattleCatalog) -> bool {
    let [circle_id, level] = slot.behavior.args.as_slice() else {
        return false;
    };
    slot.behavior.spec.kind == BehaviorKind::AddMagicCircle
        && !slot.conditions.is_empty()
        && slot
            .conditions
            .iter()
            .all(|condition| matches!(condition.kind, ParsedConditionKind::NotInMagicCircleId(_)))
        && catalog.is_initial_electric_field(*circle_id, *level)
}

#[derive(Default)]
pub(crate) struct OpeningFields {
    assigned: Vec<SetupSubscriber>,
}

impl OpeningFields {
    pub(crate) fn deploy(
        &mut self,
        managers: &mut BattleManagers,
        pool: &TargetPool,
        catalog: &SkillEffectCatalog,
        determinism: &mut RoundDeterminism,
        context: TargetContext,
        owner_uids: &[i64],
    ) -> Result<DrainResult, DrainError> {
        let assigned = RefCell::new(&mut self.assigned);
        let battle_catalog = managers.catalog();
        let select = |subscriber: &SetupSubscriber, slot: &SkillEffectSlot| {
            if !owns(slot, battle_catalog) {
                return false;
            }
            assigned.borrow_mut().push(*subscriber);
            true
        };
        run_skill_then_buff_act_setup_stage_filtered(
            BattleAccess::new(managers, pool, catalog, determinism, context),
            crate::engine::runtime::drain::entry::setup::SetupSelection {
                stage: SetupStage::RoundStartCondition,
                priority: 100,
                owner_uids: Some(owner_uids),
                excluded_owner_uids: &[],
            },
            false,
            SetupSkillFilter::Select(&select),
            false,
            SetupFrameContainer::Standalone,
            DefenderCardCleanupOwner::DeathAction,
        )
    }

    pub(crate) fn includes_remaining(&self, subscriber: &SetupSubscriber) -> bool {
        !self.assigned.contains(subscriber)
    }
}
