use super::{BuffApplyResult, BuffManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayBuffOwner {
    pub(crate) team: i32,
    pub(crate) field_id: i32,
    pub(crate) caster_uid: i64,
}

impl BuffManager {
    pub(super) fn attach_array_owner(&mut self, added: &BuffApplyResult, owner: ArrayBuffOwner) {
        if let Some(active) = self.buffs.iter_mut().find(|active| {
            active.owner_uid == added.target_uid && active.buff.uid == added.buff.uid
        }) {
            active.array_owner = Some(owner);
        }
    }

    pub(crate) fn array_buff_uids(
        &self,
        owner: ArrayBuffOwner,
    ) -> impl Iterator<Item = (i64, i64)> + '_ {
        self.buffs.iter().filter_map(move |active| {
            if active.array_owner == Some(owner) {
                Some((active.owner_uid, active.buff.uid?))
            } else {
                None
            }
        })
    }
}
