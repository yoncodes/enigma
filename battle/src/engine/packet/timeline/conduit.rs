use sonettobuf::ActEffect;

use crate::engine::{manager::conduit::ConduitChange, packet::effect::EffectPacket};

pub(super) fn project_conduit(change: &ConduitChange) -> Vec<ActEffect> {
    match change {
        ConduitChange::Initialized(area) => vec![EffectPacket::conduit_initialized(area)],
        ConduitChange::RosterRefreshed(area) => vec![EffectPacket::conduit_initialized(area)],
        ConduitChange::SelectionCommitted {
            source_uid,
            team,
            group,
        } => vec![EffectPacket::conduit_group_selected(
            *source_uid,
            *team,
            *group,
            0,
        )],
        ConduitChange::SkillGroupChanged {
            origin,
            source_uid,
            team,
            group,
        } => vec![EffectPacket::conduit_group_selected(
            *source_uid,
            *team,
            *group,
            origin.key.opcode,
        )],
        ConduitChange::SkillBegan {
            team,
            power_id,
            activation_cost,
            spent,
            generic_spent,
            ..
        } if *power_id != 999 && (*spent > 0 || *activation_cost == 0) => {
            vec![EffectPacket::conduit_skill_began(
                *team,
                *power_id,
                spent.saturating_sub(*generic_spent),
                *generic_spent,
            )]
        }
        ConduitChange::SkillBegan { .. } => Vec::new(),
        ConduitChange::SkillCostCommitted {
            source_uid,
            team,
            power_id,
            consumed_this_round,
            ..
        } if *power_id != 999 => vec![EffectPacket::conduit_skill_cost_committed(
            *source_uid,
            *team,
            *consumed_this_round,
        )],
        ConduitChange::SkillCostCommitted { .. } => Vec::new(),
        ConduitChange::SkillFinished {
            source_uid,
            team,
            uses_this_round,
            ..
        } => vec![EffectPacket::conduit_skill_finished(
            *source_uid,
            *team,
            *uses_this_round,
        )],
        ConduitChange::ActivationCompleted(_) => Vec::new(),
        ConduitChange::RunningChanged { running, .. } => {
            vec![EffectPacket::conduit_running(*running)]
        }
        ConduitChange::PowerChanged {
            team,
            power_id,
            applied_delta,
            kind,
            ..
        } => vec![EffectPacket::conduit_power_changed(
            *team,
            *power_id,
            *applied_delta,
            *kind,
        )],
        ConduitChange::CounterChanged {
            source_uid,
            team,
            kind,
            after,
            ..
        } => vec![EffectPacket::conduit_counter_changed(
            *source_uid,
            *team,
            *kind,
            *after,
        )],
        ConduitChange::PowersCleared {
            team,
            spent_by_power,
            ..
        } => EffectPacket::conduit_powers_consumed(*team, spent_by_power),
        ConduitChange::PowersReset { team } => vec![EffectPacket::conduit_powers_reset(*team)],
        ConduitChange::SkillStopped {
            source_uid,
            team,
            skill_id,
            ..
        } => vec![EffectPacket::conduit_skill_stopped(
            *source_uid,
            *team,
            *skill_id,
        )],
        ConduitChange::DeviceRestarted { source_uid, team } => {
            vec![EffectPacket::conduit_device_restarted(*source_uid, *team)]
        }
    }
}
