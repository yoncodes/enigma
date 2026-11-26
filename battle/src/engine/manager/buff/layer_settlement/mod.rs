use std::collections::BTreeMap;

use crate::engine::event::payload::{BattleEvent, TeamBuffLayersSettled};
use crate::engine::skill::effect::SkillEffectCatalog;

use super::{BuffDefinition, BuffManager};

#[derive(Debug, Clone, Default)]
pub(super) struct Ledger {
    layers: BTreeMap<(i32, i32), Layers>,
}

#[derive(Debug, Clone, Default)]
struct Layers {
    applied: i32,
    settled: i32,
}

impl BuffManager {
    pub(crate) fn settle_team_buff_layers(
        &mut self,
        events: &[BattleEvent],
        skills: &SkillEffectCatalog,
    ) -> Vec<BattleEvent> {
        if !skills.observes_team_buff_layers() {
            return Vec::new();
        }
        let catalog = self.catalog();
        let mut grants = Vec::new();
        for event in events {
            let (change, added) = match event {
                BattleEvent::BuffAdded(change) => (change, true),
                BattleEvent::BuffChanged(change) => (change, false),
                _ => continue,
            };
            let gained = change.after_amount.saturating_sub(change.before_amount);
            if change.added_type_total.is_none() || gained <= 0 {
                continue;
            }
            let Some(team) = self.team_type(change.source_uid) else {
                continue;
            };
            let type_id = catalog.buff_type_id(change.buff_id);
            grants.push((team, type_id, added));
            if self
                .team_type(change.target_uid)
                .is_some_and(|target| target != team)
                && skills.tracks_team_buff_layers(type_id)
                && BuffDefinition::configured(catalog.game_data(), change.buff_id)
                    .is_some_and(|definition| definition.uses_stack_layer())
            {
                let layers = self
                    .layer_settlement
                    .layers
                    .entry((team, type_id))
                    .or_default();
                layers.applied = layers.applied.saturating_add(gained);
            }
        }

        self.layer_settlement
            .layers
            .iter_mut()
            .filter_map(|(&(team, type_id), layers)| {
                if layers.applied == layers.settled
                    || !grants.iter().any(|&(grant_team, grant_type, added)| {
                        team == grant_team && (grant_type != type_id || added)
                    })
                {
                    return None;
                }
                let before = std::mem::replace(&mut layers.settled, layers.applied);
                Some(BattleEvent::TeamBuffLayersSettled(TeamBuffLayersSettled {
                    team,
                    type_id,
                    before,
                    after: layers.applied,
                }))
            })
            .collect()
    }
}
