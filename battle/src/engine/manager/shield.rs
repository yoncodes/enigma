use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffCommand, BuffCommandError, BuffGrant, BuffManager, BuffMarkerResult,
            BuffPlan, BuffPolicy, BuffRefreshDuration, BuffSetState,
        },
        hp::{HpChanges, HpCommand, HpCommandError, ShieldGrant, TeamSharedShieldGain},
    },
    skill::buff_act::registry::BuffActKind,
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldScope {
    Entity,
    TeamShared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldCarrierUid {
    Definition,
    Child,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub amount_attr: crate::engine::entity::attr::AttrId,
    pub amount_rate: i32,
    pub multiplier_bonus: Option<(crate::engine::entity::attr::AttrId, i32)>,
    pub max_attr: crate::engine::entity::attr::AttrId,
    pub max_rate: i32,
    pub scope: ShieldScope,
    pub carrier_uid: ShieldCarrierUid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldChanges {
    command: ShieldCommand,
    pub buff: Option<BuffChanges>,
    pub hp: Option<HpChanges>,
    pub team_shared: Option<TeamSharedShieldGain>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldCommandError {
    Buff(BuffCommandError),
    Hp(HpCommandError),
}

impl From<BuffCommandError> for ShieldCommandError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<HpCommandError> for ShieldCommandError {
    fn from(value: HpCommandError) -> Self {
        Self::Hp(value)
    }
}

struct PlannedBuff {
    plan: BuffPlan,
    expose: bool,
}

struct ShieldPlan {
    command: ShieldCommand,
    buff: Option<PlannedBuff>,
    hp: Option<HpCommand>,
    team_shared: Option<TeamSharedShieldGain>,
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: ShieldCommand,
) -> Result<ShieldChanges, ShieldCommandError> {
    let plan = plan(managers, command)?;
    Ok(commit(managers, plan))
}

fn plan(
    managers: &BattleManagers,
    command: ShieldCommand,
) -> Result<ShieldPlan, ShieldCommandError> {
    let carrier_uid = managers
        .buff
        .buff_family_carrier_uid(command.target_uid, command.buff_id);
    let (amount_attr, amount_rate) =
        if carrier_uid.is_none() && command.scope == ShieldScope::Entity {
            crate::engine::skill::buff_act::shield::cumulative_attr_rate(
                command.buff_id,
                &managers.buff,
            )
            .unwrap_or((command.amount_attr, command.amount_rate))
        } else {
            (command.amount_attr, command.amount_rate)
        };
    let basis = managers.origin_attribute(command.source_uid, amount_attr);
    let multiplier_bonus = command
        .multiplier_bonus
        .map(|(attr, rate)| (managers.origin_attribute(command.source_uid, attr), rate));
    let amount = shield_amount(basis, amount_rate, multiplier_bonus);
    let max = managers
        .origin_attribute(command.source_uid, command.max_attr)
        .saturating_mul(command.max_rate)
        / 1000;

    match command.scope {
        ShieldScope::Entity => {
            let buff = match carrier_uid {
                Some(buff_uid) => {
                    let carrier = managers.buff.snapshot(command.target_uid, buff_uid).ok_or(
                        ShieldCommandError::Buff(BuffCommandError::InvalidDurationChange),
                    )?;
                    let current_duration = carrier.duration.ok_or(ShieldCommandError::Buff(
                        BuffCommandError::InvalidDurationChange,
                    ))?;
                    let configured_duration = BuffPolicy::try_for_buff_id(command.buff_id)
                        .map_err(BuffCommandError::InvalidPolicy)?
                        .lifetime
                        .duration;
                    (carrier.buff_id == Some(command.buff_id)
                        && current_duration > 0
                        && configured_duration > 0)
                        .then(|| {
                            managers.plan_buff(BuffCommand::RefreshDuration(BuffRefreshDuration {
                                origin: command.origin,
                                target_uid: command.target_uid,
                                buff_uid,
                                minimum_duration: configured_duration,
                            }))
                        })
                        .transpose()?
                }
                None => Some(plan_carrier(managers, command)?),
            };
            let accepted =
                carrier_uid.is_some() || buff.as_ref().and_then(BuffPlan::added_buff_uid).is_some();
            let hp = accepted.then_some(HpCommand::GrantShield(ShieldGrant {
                origin: command.origin,
                source_uid: command.source_uid,
                target_uid: command.target_uid,
                amount,
                max,
            }));
            if let Some(hp) = hp {
                managers.hp.validate_command(hp)?;
            }
            Ok(ShieldPlan {
                command,
                buff: buff.map(|plan| PlannedBuff { plan, expose: true }),
                hp,
                team_shared: None,
            })
        }
        ShieldScope::TeamShared => {
            let act_id = configured_act_id(
                &managers.buff,
                command.buff_id,
                BuffActKind::TeamShareShield,
            )
            .ok_or(ShieldCommandError::Buff(BuffCommandError::InvalidSetState))?;
            let max = max.max(0);
            let before = carrier_uid
                .and_then(|buff_uid| managers.buff.snapshot(command.target_uid, buff_uid))
                .and_then(|buff| {
                    buff.act_info
                        .iter()
                        .find(|info| info.act_id == Some(act_id))
                        .and_then(|info| info.param.first())
                        .copied()
                })
                .unwrap_or_default();
            let after = before.saturating_add(amount.max(0)).min(max);
            let (buff, buff_uid) = if let Some(buff_uid) = carrier_uid {
                let mut snapshot = managers
                    .buff
                    .snapshot(command.target_uid, buff_uid)
                    .ok_or(ShieldCommandError::Buff(BuffCommandError::InvalidSetState))?;
                upsert_act_info(&mut snapshot.act_info, act_id, after);
                sort_act_info(&managers.buff, command.buff_id, &mut snapshot.act_info);
                (
                    Some(PlannedBuff {
                        plan: managers.plan_buff(BuffCommand::SetInternalState(BuffSetState {
                            origin: command.origin,
                            target_uid: command.target_uid,
                            buff_uid,
                            ex_info: None,
                            params: None,
                            act_info: Some(snapshot.act_info),
                        }))?,
                        expose: false,
                    }),
                    Some(buff_uid),
                )
            } else {
                let mut plan = plan_carrier(managers, command)?;
                let team_type = managers
                    .buff
                    .team_type(command.target_uid)
                    .ok_or(ShieldCommandError::Buff(BuffCommandError::InvalidSetState))?;
                let buff_uid = plan.initialize_added_act_value(
                    act_id,
                    after,
                    team_type,
                    managers.hp.current(command.target_uid),
                );
                (Some(PlannedBuff { plan, expose: true }), buff_uid)
            };
            let team_shared = buff_uid.map(|buff_uid| TeamSharedShieldGain {
                buff_uid,
                owner_uid: command.target_uid,
                buff_act_id: act_id,
                before,
                added: after - before,
                after,
                max,
            });
            Ok(ShieldPlan {
                command,
                buff,
                hp: None,
                team_shared,
            })
        }
    }
}

fn shield_amount(basis: i32, rate: i32, multiplier_bonus: Option<(i32, i32)>) -> i32 {
    let rate = i128::from(rate) * 1_000
        + multiplier_bonus.map_or(0, |(multiplier, bonus_rate)| {
            i128::from(multiplier) * i128::from(bonus_rate)
        });
    let amount = i128::from(basis) * rate / 1_000_000;
    amount.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn plan_carrier(
    managers: &BattleManagers,
    command: ShieldCommand,
) -> Result<BuffPlan, ShieldCommandError> {
    let grant = BuffGrant {
        origin: command.origin,
        source_uid: command.source_uid,
        target_uid: command.target_uid,
        buff_id: command.buff_id,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    };
    Ok(managers.plan_buff(match command.carrier_uid {
        ShieldCarrierUid::Definition => BuffCommand::Grant(grant),
        ShieldCarrierUid::Child => BuffCommand::GrantUsingChildUid(grant),
    })?)
}

fn commit(managers: &mut BattleManagers, plan: ShieldPlan) -> ShieldChanges {
    let mut buff = plan.buff.and_then(|planned| {
        let changes = managers.commit_buff(planned.plan);
        planned.expose.then_some(changes)
    });
    let hp = plan.hp.map(|command| {
        managers
            .hp
            .commit_validated_command_with_team_shared(command, None)
    });
    if let Some(gain) = plan.team_shared
        && let Some(added) = buff
            .as_mut()
            .and_then(|changes| changes.change.added.as_mut())
    {
        added.markers.insert(
            0,
            BuffMarkerResult {
                target_uid: gain.owner_uid,
                effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
                effect_num: gain.after,
                buff_act_id: 0,
            },
        );
    }
    ShieldChanges {
        command: plan.command,
        buff,
        hp,
        team_shared: plan.team_shared,
    }
}

fn configured_act_id(buffs: &BuffManager, buff_id: i32, kind: BuffActKind) -> Option<i32> {
    buffs
        .definition_features(buff_id)
        .into_iter()
        .find_map(|feature| {
            let act_id = feature.act_id()?;
            crate::engine::skill::buff_act::registry::find(act_id, &feature.act_type)
                .filter(|definition| definition.kind == kind)
                .map(|_| act_id)
        })
}

fn upsert_act_info(act_info: &mut Vec<sonettobuf::BuffActInfo>, act_id: i32, value: i32) {
    if let Some(info) = act_info.iter_mut().find(|info| info.act_id == Some(act_id)) {
        info.param = vec![value];
        info.str_param = Some(String::new());
    } else {
        act_info.push(sonettobuf::BuffActInfo {
            act_id: Some(act_id),
            param: vec![value],
            str_param: Some(String::new()),
        });
    }
}

fn sort_act_info(buffs: &BuffManager, buff_id: i32, act_info: &mut [sonettobuf::BuffActInfo]) {
    let order = buffs
        .definition_features(buff_id)
        .into_iter()
        .filter_map(|feature| feature.act_id())
        .collect::<Vec<_>>();
    act_info.sort_by_key(|info| {
        order
            .iter()
            .position(|act_id| Some(*act_id) == info.act_id)
            .unwrap_or(usize::MAX)
    });
}

#[cfg(test)]
mod tests;
