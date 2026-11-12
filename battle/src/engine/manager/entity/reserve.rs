use super::*;
use crate::engine::fight::reserve::{CycleCommand, Promotion, ReserveQueued, ReserveRetired};

impl EntityManager {
    pub(crate) fn promote_reserves(&mut self, hp: &HpManager) -> Vec<Promotion> {
        self.promote_reserves_matching(hp, |_| true)
    }

    pub(crate) fn promote_immediate_reserves(
        &mut self,
        hp: &HpManager,
        promotes_immediately: impl Fn(i32) -> bool,
    ) -> Vec<Promotion> {
        self.promote_reserves_matching(hp, promotes_immediately)
    }

    fn promote_reserves_matching(
        &mut self,
        hp: &HpManager,
        promotes: impl Fn(i32) -> bool,
    ) -> Vec<Promotion> {
        let mut promotions = Vec::new();
        for (defeated_uid, entering_uid, team_type) in self
            .pending_reserve_promotions(hp)
            .into_iter()
            .filter(|(_, _, team_type)| promotes(*team_type))
        {
            let Some(mut defeated) = self.identities.get(&defeated_uid).cloned() else {
                continue;
            };
            let Some(mut entering) = self.identities.get(&entering_uid).cloned() else {
                continue;
            };
            std::mem::swap(&mut defeated.position, &mut entering.position);
            let main_position = entering.position;
            if let (Some(defeated_index), Some(entering_index)) = (
                self.order.iter().position(|uid| *uid == defeated_uid),
                self.order.iter().position(|uid| *uid == entering_uid),
            ) {
                self.order.swap(defeated_index, entering_index);
            }
            let reserves = self.reserve_order.entry(team_type).or_default();
            reserves.retain(|uid| *uid != entering_uid && *uid != defeated_uid);
            reserves.push(defeated_uid);
            self.identities.insert(defeated_uid, defeated.clone());
            self.identities.insert(entering_uid, entering.clone());
            self.roster_lanes.insert(defeated_uid, RosterLane::Reserve);
            self.roster_lanes.insert(entering_uid, RosterLane::Main);
            promotions.push(Promotion {
                defeated_uid,
                entering_uid,
                position: main_position.unwrap_or_default(),
                team_type,
                entering,
                queued_reserve: None,
            });
        }
        promotions
    }

    pub(in crate::engine::manager) fn queue_reserve_cycle(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
        command: CycleCommand,
    ) -> ReserveQueued {
        let CycleCommand {
            defeated_uid,
            team_type,
        } = command;
        let mut reserve = self
            .identities
            .get(&defeated_uid)
            .expect("a committed promotion retains the defeated identity")
            .clone();
        let reserves = self
            .reserve_order
            .get_mut(&team_type)
            .expect("a committed promotion retains the reserve queue");
        let position = reserves
            .iter()
            .position(|uid| *uid == defeated_uid)
            .expect("a committed promotion moves the defeated unit into reserve");
        reserves.remove(position);
        self.roster_lanes
            .insert(defeated_uid, RosterLane::RetiringReserve);
        for (index, uid) in reserves.iter().enumerate() {
            self.identities
                .get_mut(uid)
                .expect("queued reserve has an identity")
                .position = Some(-((index + 1) as i32));
        }

        let uid = self.next_special_uid;
        self.next_special_uid -= 1;
        reserve.uid = Some(uid);
        reserve.position = Some(-((reserves.len() + 1) as i32));
        reserve.current_hp = reserve
            .attr
            .as_ref()
            .and_then(|attr| attr.hp)
            .or(reserve.current_hp);
        self.register_in_lane(catalog, &reserve, RosterLane::Reserve);
        ReserveQueued {
            team_type,
            entity: reserve,
        }
    }

    pub(crate) fn retire_reserve(
        &mut self,
        uid: i64,
    ) -> Result<ReserveRetired, EntityCommandError> {
        let team_type = self
            .team_type(uid)
            .ok_or(EntityCommandError::MissingSource)?;
        let lane = self
            .roster_lanes
            .get_mut(&uid)
            .ok_or(EntityCommandError::MissingSource)?;
        if *lane != RosterLane::RetiringReserve {
            return Err(EntityCommandError::InvalidCommand);
        }
        *lane = RosterLane::Replaced;
        Ok(ReserveRetired { uid, team_type })
    }

    pub(crate) fn has_pending_reserve_promotion(&self, uid: i64, hp: &HpManager) -> bool {
        self.pending_reserve_promotions(hp)
            .iter()
            .any(|(defeated_uid, _, _)| *defeated_uid == uid)
    }

    fn pending_reserve_promotions(&self, hp: &HpManager) -> Vec<(i64, i64, i32)> {
        [1, 2]
            .into_iter()
            .flat_map(|team_type| {
                self.roster_uids(team_type, RosterLane::Main)
                    .into_iter()
                    .filter(|uid| hp.current(*uid) <= 0)
                    .zip(
                        self.roster_uids(team_type, RosterLane::Reserve)
                            .into_iter()
                            .filter(|uid| hp.current(*uid) > 0),
                    )
                    .map(move |(defeated_uid, entering_uid)| {
                        (defeated_uid, entering_uid, team_type)
                    })
            })
            .collect()
    }
}
