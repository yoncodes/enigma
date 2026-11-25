use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffChanges, BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
    },
    skill::rule::CommandOrigin,
};

use super::{UpgradeApplied, UpgradeCommand, UpgradeOperation};

impl BattleManagers {
    pub(in crate::engine::manager) fn apply_upgrade(
        &mut self,
        owner_uid: i64,
        upgrade_id: i32,
        option_id: i32,
    ) -> Option<UpgradeApplied> {
        let change = self
            .execute_upgrade(UpgradeCommand {
                owner_uid,
                operation: UpgradeOperation::Select {
                    upgrade_id,
                    option_id,
                },
            })
            .ok()?;
        let origin = change.offer_origin?;
        let selection = change.selection.as_ref()?;
        let (card_changes, removed_buffs) = if self.fight_version() == 7 {
            let removed = self.remove_upgrade_buffs(owner_uid, origin, &selection.del_buff_ids)?;
            let mut cards = self.apply_upgrade_identity(owner_uid, origin, selection)?;
            cards.entity = Some(self.entity_snapshot(owner_uid)?);
            (cards, removed)
        } else {
            let cards = self.apply_upgrade_identity(owner_uid, origin, selection)?;
            let removed = self.remove_upgrade_buffs(owner_uid, origin, &selection.del_buff_ids)?;
            (cards, removed)
        };
        let granted_buffs = selection
            .add_buff_ids
            .iter()
            .map(|&buff_id| {
                self.execute_buff(BuffCommand::GrantUsingChildUid(BuffGrant {
                    origin,
                    source_uid: owner_uid,
                    target_uid: owner_uid,
                    buff_id,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                }))
                .ok()
            })
            .collect::<Option<Vec<_>>>()?;
        Some(UpgradeApplied {
            change,
            entity: self.entity_snapshot(owner_uid)?,
            removed_buffs,
            granted_buffs,
            card_changes,
        })
    }

    fn remove_upgrade_buffs(
        &mut self,
        owner_uid: i64,
        origin: CommandOrigin,
        buff_ids: &[i32],
    ) -> Option<Vec<BuffChanges>> {
        buff_ids
            .iter()
            .map(|&buff_id| {
                self.execute_buff(BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid: owner_uid,
                    selector: BuffRemoveSelector::ExactId(buff_id),
                }))
                .ok()
            })
            .collect()
    }
}
