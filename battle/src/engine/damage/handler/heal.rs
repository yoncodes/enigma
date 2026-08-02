use crate::engine::{
    damage::scale_permille,
    entity::attr::AttrId,
    manager::BattleManagers,
    skill::{
        buff_act::{is_kind, registry::BuffActKind},
        effect::ParsedBehavior,
    },
};

const BURN_BUFF_FIGHT_CONST: i32 = 29;
const BURN_HEALING_TAKEN: i32 = -150;

pub(super) fn attribute_amount(
    source_uid: i64,
    target_uid: i64,
    managers: &BattleManagers,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let [mode, attr_id, rate] = behavior.args.as_slice() else {
        return None;
    };
    let basis_uid = if *mode == 0 { source_uid } else { target_uid };
    let basis = managers.origin_attribute(basis_uid, AttrId::from_raw(*attr_id)?);
    Some(modified(
        scale_permille(basis, *rate),
        source_uid,
        target_uid,
        managers,
    ))
}

pub(super) fn two_attribute_amount(
    source_uid: i64,
    target_uid: i64,
    managers: &BattleManagers,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let [_, _, missing_rate, _, _, source_hp_rate] = behavior.args.as_slice() else {
        return None;
    };
    let target = managers.hp.get(target_uid);
    let base = scale_permille((target.max - target.current).max(0), *missing_rate)
        .saturating_add(scale_permille(managers.hp.max(source_uid), *source_hp_rate));
    Some(modified(base, source_uid, target_uid, managers))
}

pub(crate) fn modified(
    base: i32,
    source_uid: i64,
    target_uid: i64,
    managers: &BattleManagers,
) -> i32 {
    if base <= 0 {
        return 0;
    }
    let healing_done = managers.attribute.get(source_uid, AttrId::HealingDone)
        + managers
            .buff
            .attribute_delta(source_uid, AttrId::HealingDone);
    let target = managers.hp.get(target_uid);
    let missing_permille = if target.max > 0 {
        ((i64::from((target.max - target.current).max(0)) * 1000) / i64::from(target.max))
            .clamp(0, i64::from(i32::MAX)) as i32
    } else {
        0
    };
    let injury = managers
        .buff
        .buff_act_scalar(target_uid, BuffActKind::Injury);
    let healing_taken: i32 = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == target_uid && is_kind(feature, BuffActKind::CureUpByLostHp)
        })
        .filter_map(|feature| {
            let [_, base, per_bucket, max_buckets, bucket_size, ..] = feature.values.as_slice()
            else {
                return None;
            };
            Some(
                base + per_bucket
                    * (missing_permille / (*bucket_size).max(1)).min((*max_buckets).max(0)),
            )
        })
        .sum::<i32>()
        + burn_type_id()
            .filter(|type_id| {
                managers
                    .buff
                    .has_active_buff_id_or_type(target_uid, *type_id)
            })
            .map_or(0, |_| BURN_HEALING_TAKEN);
    let healing_taken = healing_taken.saturating_sub(injury);
    scale_permille(
        scale_permille(base, 1000_i32.saturating_add(healing_done)),
        1000_i32.saturating_add(healing_taken),
    )
    .max(1)
}

pub(super) fn burn_type_id() -> Option<i32> {
    config::configs::get()
        .fight_const
        .get(BURN_BUFF_FIGHT_CONST)?
        .value
        .parse()
        .ok()
}

pub(super) fn amount(
    source_uid: i64,
    target_uid: i64,
    managers: &BattleManagers,
    is_crit: bool,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let amount = if let [mode, attr_id, rate] = behavior.args.as_slice() {
        let basis_uid = if *mode == 0 { source_uid } else { target_uid };
        let basis = managers.origin_attribute(basis_uid, AttrId::from_raw(*attr_id)?);
        modified(
            scale_permille(basis, *rate),
            source_uid,
            target_uid,
            managers,
        )
    } else {
        behavior.args.first().copied()?
    };
    Some(if is_crit {
        scale_permille(
            amount,
            managers.attribute.get(source_uid, AttrId::CriticalDmg),
        )
    } else {
        amount
    })
}
