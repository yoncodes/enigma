use crate::engine::{
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffDurationAdvance},
        hp::{HpCommand, HpHeal, HpHealKind},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let kind = super::subscriber_kind(subscriber)?;
    if !supports(kind, &subscriber.args) {
        return None;
    }
    let attacked = matches!(event, BattleEvent::Hit(_));
    match kind {
        BuffActKind::Cure
            if !matches!(
                event,
                BattleEvent::RoundStart | BattleEvent::Kind(EventKind::RoundStart)
            ) =>
        {
            return Some(Vec::new());
        }
        BuffActKind::AdvancedCure
            if !matches!(
                event,
                BattleEvent::RoundStart
                    | BattleEvent::Kind(EventKind::RoundStart)
                    | BattleEvent::Hit(_)
            ) =>
        {
            return Some(Vec::new());
        }
        _ => {}
    }
    // A holder killed by the hit is not healed back; revival is its own act.
    if attacked
        && (event.target_uid() != Some(subscriber.owner_uid)
            || managers.hp.current(subscriber.owner_uid) <= 0
            || managers
                .hp
                .current(subscriber.owner_uid)
                .saturating_mul(1000)
                >= managers
                    .hp
                    .max(subscriber.owner_uid)
                    .saturating_mul(subscriber.args[3]))
    {
        return Some(Vec::new());
    }
    let Some(command) = heal_command(managers, subscriber, subscriber.key.definition.opcode) else {
        return Some(Vec::new());
    };
    let mut ops = vec![RuleOp::Command(BattleCommand::Hp(command))];
    if attacked {
        let take_stage = managers
            .buff
            .duration_stage(subscriber.owner_uid, subscriber.buff_uid)?;
        let advance = BuffDurationAdvance::new(
            take_stage,
            vec![subscriber.owner_uid],
            Some(vec![subscriber.buff_uid]),
        )?;
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AdvanceDuration(advance),
        )));
    }
    Some(ops)
}

pub(crate) fn heal_command(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    config_effect: i32,
) -> Option<HpCommand> {
    let source_uid = if subscriber.source_uid != 0 {
        subscriber.source_uid
    } else {
        subscriber.owner_uid
    };
    let kind = super::subscriber_kind(subscriber)?;
    let base = match kind {
        BuffActKind::Cure => subscriber
            .args
            .chunks_exact(3)
            .try_fold(0_i32, |sum, chunk| {
                let attr = AttrId::from_raw(chunk[1])?;
                Some(
                    sum.saturating_add(
                        managers
                            .origin_attribute(subscriber.owner_uid, attr)
                            .max(0)
                            .saturating_mul(chunk[2])
                            / 1000,
                    ),
                )
            })?,
        BuffActKind::AdvancedCure => {
            let [_mode, raw_attr, permille, _threshold] = subscriber.args.as_slice() else {
                return None;
            };
            managers
                .origin_attribute(source_uid, AttrId::from_raw(*raw_attr)?)
                .max(0)
                .saturating_mul(*permille)
                / 1000
        }
        _ => return None,
    };
    let amount = crate::engine::damage::handler::modified_heal(
        base,
        source_uid,
        subscriber.owner_uid,
        managers,
        &[],
    );
    (amount > 0).then(|| {
        HpCommand::Heal(HpHeal {
            origin: super::command_origin(subscriber).expect("registered Cure buff act"),
            source_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect,
            kind: HpHealKind::Normal,
        })
    })
}

pub fn supports(kind: BuffActKind, args: &[i32]) -> bool {
    match kind {
        BuffActKind::Cure => {
            !args.is_empty()
                && args.len().is_multiple_of(3)
                && args
                    .chunks_exact(3)
                    .all(|chunk| AttrId::from_raw(chunk[1]).is_some() && chunk[2] > 0)
        }
        BuffActKind::AdvancedCure => matches!(
            args,
            [_, raw_attr, permille, threshold]
                if AttrId::from_raw(*raw_attr).is_some() && *permille > 0 && *threshold > 0
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
