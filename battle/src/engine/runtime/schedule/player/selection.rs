use super::*;
use crate::engine::manager::conduit::ConduitCommand;

#[derive(Default)]
pub(in crate::engine::runtime::schedule) struct Selection(Vec<(i64, i32)>);

impl Selection {
    pub(super) fn from_operations(operations: &[sonettobuf::FightDeviceOper]) -> Self {
        Self(
            operations
                .iter()
                .filter_map(|operation| Some((operation.uid?, operation.index?)))
                .collect(),
        )
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn commit(
        &self,
        managers: &mut BattleManagers,
        pool: &TargetPool,
        catalog: &SkillEffectCatalog,
        determinism: &mut RoundDeterminism,
        context: TargetContext,
    ) -> Result<DrainResult, DrainError> {
        if self.is_empty() {
            return Ok(DrainResult::default());
        }
        drain::run_command_group_as(
            managers,
            pool,
            catalog,
            determinism,
            context,
            FrameOwner::ConduitSelectionCommit,
            self.0.iter().map(|&(source_uid, group)| {
                RuleOp::Command(BattleCommand::Conduit(ConduitCommand::SelectGroup {
                    source_uid,
                    group,
                }))
            }),
        )
    }
}
