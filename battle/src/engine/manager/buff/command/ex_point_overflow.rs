use crate::engine::skill::buff_act::ex_point_overflow_bank::credit;

use super::{BuffBankExPointOverflow, BuffCommandError, BuffManager, SetStatePlan};

impl BuffManager {
    pub(super) fn plan_ex_point_overflow(
        &self,
        update: BuffBankExPointOverflow,
    ) -> Result<SetStatePlan, BuffCommandError> {
        if update.target_uid == 0
            || update.buff_uid <= 0
            || update.overflow <= 0
            || update.capacity <= 0
        {
            return Err(BuffCommandError::InvalidSetState);
        }
        let params = self
            .snapshot(update.target_uid, update.buff_uid)
            .and_then(|buff| {
                credit(
                    &buff,
                    update.origin.key.opcode,
                    update.overflow,
                    update.capacity,
                )
            });
        Ok(SetStatePlan {
            target_uid: update.target_uid,
            buff_uid: update.buff_uid,
            ex_info: None,
            exists: params.is_some(),
            params,
            act_info: None,
        })
    }
}
