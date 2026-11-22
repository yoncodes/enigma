use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::kind::EventKind,
    manager::card::{CardCommand, HandCardRankUp},
    skill::rule::output::{BattleCommand, RuleOp},
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [mode] if matches!(*mode, 1 | 2 | 9))
}

pub fn rule_ops(context: &mut super::registry::RuntimeContext<'_>) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(context.subscriber, BuffActKind::CardLevelAdd) {
        return None;
    }
    let [mode] = context.subscriber.args.as_slice() else {
        return None;
    };
    if !supports(context.subscriber.args.as_slice()) {
        return None;
    }
    if context.event?.kind() != EventKind::RoundStart {
        return Some(Vec::new());
    }

    let mut candidates = context
        .managers
        .card
        .hand_rank_up_candidates(context.subscriber.owner_uid);
    if candidates.is_empty() {
        return Some(Vec::new());
    }

    let count = match *mode {
        1 => 1,
        2 => 2,
        9 => candidates.len(),
        _ => unreachable!("CardLevelAdd arguments were validated above"),
    };
    let origin = super::command_origin(context.subscriber)?;
    let mut ops = Vec::with_capacity(count.saturating_mul(2));

    for _ in 0..count {
        let hand_index = context
            .determinism
            .take_hand_rank_choice(
                context.subscriber.key.definition.opcode,
                context.subscriber.owner_uid,
                &candidates,
            )
            .or_else(|| {
                context
                    .determinism
                    .lua_random_index(candidates.len())
                    .map(|chosen| candidates[chosen])
            })?;
        let chosen = candidates
            .iter()
            .position(|candidate| *candidate == hand_index)?;

        ops.push(RuleOp::BuffFeatureMarker {
            target_uid: context.subscriber.owner_uid,
            effect_type: EffectType::Cardleveladd as i32,
            effect_num: context.subscriber.buff_id,
            buff_act_id: context.subscriber.key.definition.opcode,
        });
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::RankUpHand(HandCardRankUp {
                origin,
                owner_uid: context.subscriber.owner_uid,
                hand_index,
            }),
        )));
        candidates.swap_remove(chosen);
        if candidates.is_empty() {
            break;
        }
    }

    Some(ops)
}

#[cfg(test)]
mod tests;
