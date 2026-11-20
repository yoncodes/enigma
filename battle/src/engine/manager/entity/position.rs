use crate::engine::skill::rule::CommandOrigin;

use super::{EntityManager, RosterLane, SPECIAL_POSITION};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveFrontCommand {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub distance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionChange {
    pub uid: i64,
    pub before: i32,
    pub after: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveFrontChanges {
    pub origin: CommandOrigin,
    pub target: PositionChange,
    pub displaced: Vec<PositionChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveFrontError {
    InvalidCommand,
    MissingTarget,
}

impl EntityManager {
    pub(crate) fn execute_move_front(
        &mut self,
        command: MoveFrontCommand,
    ) -> Result<MoveFrontChanges, MoveFrontError> {
        if command.distance <= 0 {
            return Err(MoveFrontError::InvalidCommand);
        }
        let target = self
            .identities
            .get(&command.target_uid)
            .ok_or(MoveFrontError::MissingTarget)?;
        let team = self
            .teams
            .get(&command.target_uid)
            .copied()
            .ok_or(MoveFrontError::MissingTarget)?;
        let before = target.position.ok_or(MoveFrontError::InvalidCommand)?;
        if self.roster_lanes.get(&command.target_uid) != Some(&RosterLane::Main)
            || !(1..SPECIAL_POSITION).contains(&before)
        {
            return Err(MoveFrontError::InvalidCommand);
        }

        let after = before.saturating_sub(command.distance).max(1);
        let mut displaced = self
            .identities
            .iter()
            .filter_map(|(&uid, entity)| {
                let position = entity.position?;
                (uid != command.target_uid
                    && self.teams.get(&uid) == Some(&team)
                    && self.roster_lanes.get(&uid) == Some(&RosterLane::Main)
                    && (after..before).contains(&position))
                .then_some(PositionChange {
                    uid,
                    before: position,
                    after: position + 1,
                })
            })
            .collect::<Vec<_>>();
        displaced.sort_by_key(|change| (std::cmp::Reverse(change.before), change.uid));

        for change in &displaced {
            self.identities
                .get_mut(&change.uid)
                .expect("position owner collected an existing entity")
                .position = Some(change.after);
        }
        self.identities
            .get_mut(&command.target_uid)
            .expect("position owner validated the target")
            .position = Some(after);

        Ok(MoveFrontChanges {
            origin: command.origin,
            target: PositionChange {
                uid: command.target_uid,
                before,
                after,
            },
            displaced,
        })
    }
}
