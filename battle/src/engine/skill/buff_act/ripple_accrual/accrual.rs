use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffAccumulateCappedActState, BuffChanges, BuffCommand, BuffCommandError, BuffGrant,
            GrantInputs,
        },
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accrue {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub receiver: i32,
    pub act_id: i32,
    pub amount: i32,
    pub capacity: i32,
}

impl Accrue {
    fn accumulate(self, buff_uid: i64) -> BuffCommand {
        BuffCommand::AccumulateCappedActState(BuffAccumulateCappedActState {
            origin: self.origin,
            target_uid: self.target_uid,
            buff_uid,
            act_id: self.act_id,
            delta: self.amount,
            maximum: self.capacity,
        })
    }

    pub(crate) fn commit(
        self,
        managers: &mut BattleManagers,
    ) -> Result<Vec<BuffChanges>, BuffCommandError> {
        if managers.hp.current(self.target_uid) <= 0 {
            return Ok(Vec::new());
        }
        if let Some(uid) = managers.buff.buff_id_uid(self.target_uid, self.receiver) {
            return managers
                .execute_buff(self.accumulate(uid))
                .map(|update| vec![update]);
        }

        let grant = managers.plan_buff(BuffCommand::Grant(BuffGrant {
            origin: self.origin,
            source_uid: self.source_uid,
            target_uid: self.target_uid,
            buff_id: self.receiver,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))?;
        let mut staged = managers.buff.clone();
        let planned = staged.commit(&managers.hp, grant.clone());
        let Some(uid) = planned
            .change
            .added
            .as_ref()
            .and_then(|added| added.buff.uid)
        else {
            return Ok(vec![managers.commit_buff(grant)]);
        };
        let update = staged.plan_with_source_attack(
            &managers.hp,
            self.accumulate(uid),
            GrantInputs::default(),
        )?;
        Ok(vec![
            managers.commit_buff(grant),
            managers.commit_buff(update),
        ])
    }
}
