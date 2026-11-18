use super::super::{BuffManager, BuffPolicy, BuffRoute, BuffStorage, DuplicateGrant};

impl BuffManager {
    pub(crate) fn replacement_uid(
        &self,
        source_uid: i64,
        target_uid: i64,
        buff_id: i32,
        removed_uid: i64,
    ) -> Option<i64> {
        let policy = BuffPolicy::configured(self.catalog().game_data(), buff_id).ok()?;
        if policy.storage != BuffStorage::Single
            || policy.on_duplicate != DuplicateGrant::ReplaceExisting
        {
            return None;
        }
        let route = BuffRoute::new(source_uid, target_uid, buff_id);
        self.buffs
            .iter()
            .find(|active| {
                active.effective
                    && active.buff.uid != Some(removed_uid)
                    && policy.matches(active, route)
            })
            .and_then(|active| active.buff.uid)
    }
}
