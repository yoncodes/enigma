use crate::{
    error::AppError,
    red_dot,
    reward::{self, AppliedRewards},
};
use database::db::game::mail;
use sonettobuf::{
    DeleteMailBatchReply, GetAllMailsReply, Mail, MailLockReply, MarkMailJumpReply,
    ReadMailBatchReply, ReadMailReply,
};
use sqlx::SqlitePool;

#[derive(Default)]
pub struct MailClaimOutcome {
    pub incr_ids: Vec<i64>,
    pub rewards: AppliedRewards,
    pub material_changes: Vec<(u32, u32, i32)>,
    pub mail_red_dot: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct MailManager {
    player_id: i64,
}

impl MailManager {
    pub fn new(player_id: i64) -> Self {
        Self { player_id }
    }

    pub async fn get_all(&self, db: &SqlitePool) -> Result<GetAllMailsReply, AppError> {
        Ok(GetAllMailsReply {
            mails: mail::get_all(db, self.player_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub async fn created_since(&self, db: &SqlitePool, since: i64) -> Result<Vec<Mail>, AppError> {
        Ok(mail::get_created_since(db, self.player_id, since)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn set_lock(
        &self,
        db: &SqlitePool,
        incr_id: i64,
        lock: bool,
    ) -> Result<MailLockReply, AppError> {
        if !mail::set_lock(db, self.player_id, incr_id, lock).await? {
            return Err(AppError::InvalidRequest);
        }

        Ok(MailLockReply {
            incr_id: Some(incr_id as u64),
            lock: Some(lock),
        })
    }

    pub async fn delete_claimed_unlocked(
        &self,
        db: &SqlitePool,
    ) -> Result<DeleteMailBatchReply, AppError> {
        Ok(DeleteMailBatchReply {
            incr_ids: mail::delete_claimed_unlocked(db, self.player_id)
                .await?
                .into_iter()
                .map(|id| id as u64)
                .collect(),
        })
    }

    pub async fn mark_jump(
        &self,
        db: &SqlitePool,
        incr_id: i64,
    ) -> Result<MarkMailJumpReply, AppError> {
        if !mail::mark_jump(db, self.player_id, incr_id).await? {
            return Err(AppError::InvalidRequest);
        }
        Ok(MarkMailJumpReply {
            incr_id: Some(incr_id as u64),
        })
    }

    pub async fn claim_one(
        &self,
        db: &SqlitePool,
        incr_id: i64,
    ) -> Result<(ReadMailReply, MailClaimOutcome), AppError> {
        let Some(mail) = mail::get_by_incr_id(db, self.player_id, incr_id).await? else {
            return Err(AppError::InvalidRequest);
        };

        let outcome = if mail.state == 1 {
            MailClaimOutcome {
                incr_ids: vec![incr_id],
                ..Default::default()
            }
        } else {
            self.claim_mails(db, vec![(mail.incr_id, mail.attachment)])
                .await?
        };

        Ok((
            ReadMailReply {
                incr_id: Some(incr_id as u64),
            },
            outcome,
        ))
    }

    pub async fn claim_batch(
        &self,
        db: &SqlitePool,
    ) -> Result<(ReadMailBatchReply, MailClaimOutcome), AppError> {
        let mails = mail::get_claimable(db, self.player_id)
            .await?
            .into_iter()
            .map(|mail| (mail.incr_id, mail.attachment))
            .collect::<Vec<_>>();

        let outcome = self.claim_mails(db, mails).await?;
        let reply = ReadMailBatchReply {
            incr_ids: outcome.incr_ids.iter().map(|id| *id as u64).collect(),
        };

        Ok((reply, outcome))
    }

    async fn claim_mails(
        &self,
        db: &SqlitePool,
        mails: Vec<(i64, String)>,
    ) -> Result<MailClaimOutcome, AppError> {
        if mails.is_empty() {
            return Ok(MailClaimOutcome::default());
        }

        let red_dots = red_dot::RedDotManager::new(self.player_id);
        let previous_mail_red_dot = red_dots.mail_state(db).await?;

        let mut rewards = reward::RewardSet::default();
        for (_, attachment) in &mails {
            rewards.extend(reward::parse(attachment));
        }
        let incr_ids = mails
            .iter()
            .map(|(incr_id, _)| *incr_id)
            .collect::<Vec<_>>();
        let material_changes = rewards.material_changes();
        let mut tx = db.begin().await?;
        mail::mark_claimed_in_transaction(&mut tx, self.player_id, &incr_ids).await?;
        let applied_rewards =
            reward::apply_in_transaction(&mut tx, db, self.player_id, rewards).await?;
        tx.commit().await?;
        let mail_red_dot = red_dots.mail_state(db).await?;

        Ok(MailClaimOutcome {
            incr_ids,
            rewards: applied_rewards,
            material_changes,
            mail_red_dot: (previous_mail_red_dot.0 != mail_red_dot.0).then_some(mail_red_dot),
        })
    }
}

#[cfg(test)]
mod test;
