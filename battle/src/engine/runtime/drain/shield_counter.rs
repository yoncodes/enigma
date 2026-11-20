use crate::engine::{
    runtime::record::FramePath,
    skill::{action::ActionEvent, buff_act::shield_counter::Credit},
};

use super::*;

#[derive(Default)]
pub(super) struct Tally {
    chains: HashMap<FramePath, Vec<(Credit, usize)>>,
}

impl Tally {
    pub(super) fn earn(&mut self, chain: FramePath, credits: impl Iterator<Item = Credit>) {
        for credit in credits {
            let pending = self.chains.entry(chain.clone()).or_default();
            if let Some((_, count)) = pending.iter_mut().find(|(entry, _)| *entry == credit) {
                *count += 1;
            } else {
                pending.push((credit, 1));
            }
        }
    }

    pub(super) fn claim(
        &mut self,
        chain: &FramePath,
        path: &FramePath,
        action: &ActionEvent,
        managers: &BattleManagers,
    ) -> Vec<QueuedOp> {
        let mut claimed = Vec::new();
        if action.is_attack
            && let Some(team) = managers.entity.team_type(action.source_uid)
            && let Some(pending) = self.chains.get_mut(chain)
        {
            pending.retain(|&(credit, count)| {
                if credit.team == team {
                    return true;
                }
                if managers.hp.current(credit.owner_uid) > 0
                    && managers
                        .buff
                        .is_effective(credit.owner_uid, credit.buff_uid)
                {
                    claimed.extend(casts(credit, count, action.source_uid, path));
                }
                false
            });
        }
        self.close(path);
        claimed
    }

    pub(super) fn close(&mut self, path: &FramePath) {
        self.chains.remove(path);
    }
}

fn casts(
    credit: Credit,
    count: usize,
    attacker: i64,
    path: &FramePath,
) -> impl Iterator<Item = QueuedOp> {
    let group = Rc::new(RefCell::new(None));
    (0..count).map(move |_| QueuedOp {
        op: RuleOp::Skill(credit.cast(attacker)),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: None,
        parent_path: Some(path.clone()),
        frame_group: Some(group.clone()),
        independent_parent_group: None,
        frame_owner: Some(FrameOwner::BuffAct {
            owner_uid: credit.owner_uid,
            source_uid: credit.owner_uid,
            buff_uid: credit.buff_uid,
            buff_id: credit.buff_id,
            key: credit.key,
        }),
        subscriber: None,
        caster_frame: None,
    })
}
