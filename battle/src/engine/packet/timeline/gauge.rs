use sonettobuf::ActEffect;

use crate::engine::{
    manager::gauge::{GaugeChange, GaugeChangeKind, GaugeKind, GaugeOwner},
    packet::effect::EffectPacket,
};

use super::ProjectionError;

pub(super) fn project_gauge(change: &GaugeChange) -> Result<Vec<ActEffect>, ProjectionError> {
    Ok(match (change.key.kind, change.key.owner) {
        (GaugeKind::TeamEnergy, GaugeOwner::Team(team)) => (change.applied_delta != 0)
            .then(|| EffectPacket::team_energy_change(team, change.applied_delta))
            .into_iter()
            .collect(),
        (GaugeKind::ImpromptuInspiration, _) => (change.kind == GaugeChangeKind::Snapshot
            || change.applied_delta != 0)
            .then(|| {
                EffectPacket::emitter_energy_change(if change.kind == GaugeChangeKind::Snapshot {
                    change.after
                } else {
                    change.applied_delta
                })
            })
            .into_iter()
            .collect(),
        (GaugeKind::Bloodtithe, GaugeOwner::Team(team)) => match change.kind {
            GaugeChangeKind::Enabled if !change.enabled_before && change.enabled_after => vec![
                EffectPacket::blood_pool_max_create(team, change.config_effect, 0),
                EffectPacket::blood_pool_max_change(team, change.after_max.unwrap_or_default()),
            ],
            GaugeChangeKind::Max if change.applied_delta != 0 => {
                vec![EffectPacket::blood_pool_max_change(
                    team,
                    change.applied_delta,
                )]
            }
            GaugeChangeKind::Value if change.applied_delta != 0 => {
                vec![EffectPacket::blood_pool_value_change(
                    change.source_uid,
                    team,
                    change.applied_delta,
                    change.config_effect,
                )]
            }
            _ => Vec::new(),
        },
        (GaugeKind::LingeringGlow, GaugeOwner::Team(team)) => match change.kind {
            GaugeChangeKind::Enabled if !change.enabled_before && change.enabled_after => {
                vec![EffectPacket::blood_pool_max_create(
                    team,
                    change.config_effect,
                    change.after_max.unwrap_or_default(),
                )]
            }
            GaugeChangeKind::Value if change.applied_delta != 0 => {
                vec![EffectPacket::blood_pool_value_change(
                    change.source_uid,
                    team,
                    change.applied_delta,
                    change.config_effect,
                )]
            }
            _ => Vec::new(),
        },
        _ => return Err(ProjectionError::Gauge(change.key.kind)),
    })
}
