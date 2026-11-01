use crate::engine::manager::{
    buff::{ActiveBuffFeature, BuffManager},
    emitter,
    hp::HpManager,
};

use super::{is_kind, registry::BuffActKind};

const BASE_ATTACKS: i32 = 1;
const DEFAULT_SPLIT_DAMAGE_REDUCTION: i32 = 700;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [count] if *count > 0)
}

pub fn attack_count(buffs: &BuffManager, hp: &HpManager) -> i32 {
    attack_count_for(buffs, hp, emitter::UID)
}

pub fn attack_count_for(buffs: &BuffManager, hp: &HpManager, emitter_uid: i64) -> i32 {
    let direct = buffs
        .active_features(hp)
        .iter()
        .filter(|feature| feature.owner_uid == emitter_uid)
        .map(attack_count_delta)
        .sum::<i32>();

    (BASE_ATTACKS + direct).max(BASE_ATTACKS)
}

pub(crate) fn attack_count_for_inspiration(
    managers: &crate::engine::manager::BattleManagers,
    emitter_uid: i64,
    inspiration: i32,
) -> i32 {
    let features = managers.buff.active_features_for_owner(&managers.hp, emitter_uid);
    let thresholds = features.iter()
        .filter(|feature| is_kind(feature, BuffActKind::EmitterEnergyAddBuff))
        .filter_map(|feature| match feature.values.as_slice() {
            [893, energy, buff_id] => Some((*energy, *buff_id)),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    // Threshold buffs all share a replacement group. Derive their cumulative
    // contribution from current inspiration, without counting a materialized
    // threshold buff again or retaining last round's higher threshold.
    let direct = features.iter()
        .filter(|feature| !thresholds.iter().any(|(_, id)| *id == feature.buff_id))
        .map(attack_count_delta)
        .sum::<i32>();
    let threshold_count = thresholds.into_iter()
        .filter(|(energy, _)| inspiration >= *energy)
        .flat_map(|(_, buff_id)| managers.catalog().buff_features(buff_id))
        .filter_map(|feature| match feature.values.as_slice() {
            [878, count] => Some((*count).max(0)),
            _ => None,
        })
        .sum::<i32>();
    BASE_ATTACKS.saturating_add(direct).saturating_add(threshold_count).max(BASE_ATTACKS)
}

pub(crate) fn limit_attacks(
    attacks: i32,
    values: impl IntoIterator<Item = Vec<i32>>,
) -> (i32, i32) {
    let limit = values.into_iter().find_map(|values| match values.as_slice() {
        [1080, max_attacks, rate, max_overflow]
            if *max_attacks > 0 && *rate >= 0 && *max_overflow >= 0 =>
                Some((*max_attacks, *rate, *max_overflow)),
        _ => None,
    });
    match limit {
        Some((max_attacks, rate, max_overflow)) => (
            attacks.min(max_attacks),
            (attacks - max_attacks).clamp(0, max_overflow).saturating_mul(rate),
        ),
        None => (attacks, 0),
    }
}

pub(crate) fn split_final_damage_delta_with_circle(
    managers: &crate::engine::manager::BattleManagers,
    team: i32,
    emitter_uid: i64,
    split_count: i32,
) -> i32 {
    let correction = managers.field.emitter_features(managers.catalog(), team).into_iter()
        .filter_map(|feature| match feature.values.as_slice() {
            [1081, correction] => Some(*correction),
            _ => None,
        }).sum::<i32>();
    (split_final_damage_delta_for(&managers.buff, &managers.hp, emitter_uid, split_count)
        + if split_count > 0 { correction } else { 0 }).min(0)
}

pub fn split_count(buffs: &BuffManager, hp: &HpManager, attack_index: i32) -> i32 {
    split_count_for(buffs, hp, emitter::UID, attack_index)
}

pub fn split_count_for(
    buffs: &BuffManager,
    hp: &HpManager,
    emitter_uid: i64,
    attack_index: i32,
) -> i32 {
    let features = buffs.active_features(hp);
    let split_count = features
        .iter()
        .filter(|feature| feature.owner_uid == emitter_uid)
        .map(|feature| split_count_on_attack(feature, attack_index))
        .sum::<i32>();
    if split_count == 0 {
        return 0;
    }

    split_count
        + features
            .iter()
            .filter(|feature| feature.owner_uid == emitter_uid)
            .map(additional_split_count)
            .sum::<i32>()
}

pub fn split_final_damage_delta(buffs: &BuffManager, hp: &HpManager, split_count: i32) -> i32 {
    split_final_damage_delta_for(buffs, hp, emitter::UID, split_count)
}

pub fn split_final_damage_delta_for(
    buffs: &BuffManager,
    hp: &HpManager,
    emitter_uid: i64,
    split_count: i32,
) -> i32 {
    if split_count <= 0 {
        return 0;
    }
    let features = buffs.active_features(hp);
    let configured_reduction = features
        .iter()
        .filter(|feature| feature.owner_uid == emitter_uid)
        .filter(|feature| is_kind(feature, BuffActKind::EmitterFixSubTargetsDamageReduceRate))
        .filter_map(|feature| feature.values.get(1))
        .sum::<i32>();
    -(DEFAULT_SPLIT_DAMAGE_REDUCTION - configured_reduction).max(0)
}

fn attack_count_delta(feature: &ActiveBuffFeature) -> i32 {
    if !is_kind(feature, BuffActKind::EmitterNumChange) {
        return 0;
    }
    feature.values.get(1).copied().unwrap_or_default().max(0) * feature.amount.max(1)
}

fn split_count_on_attack(feature: &ActiveBuffFeature, attack_index: i32) -> i32 {
    let [_, start, interval, count, ..] = feature.values.as_slice() else {
        return 0;
    };
    if !is_kind(feature, BuffActKind::AttackNumSplitEmitterNum)
        || *count <= 0
        || *interval <= 0
        || attack_index < *start
        || (attack_index - start) % interval != 0
    {
        return 0;
    }
    count * feature.amount.max(1)
}

fn additional_split_count(feature: &ActiveBuffFeature) -> i32 {
    if !is_kind(feature, BuffActKind::AddSplitEmitterNum) {
        return 0;
    }
    feature.values.get(1).copied().unwrap_or_default().max(0) * feature.amount.max(1)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;

    #[test]
    fn exact_emitter_count_route_requires_one_positive_count() {
        use super::super::registry::{BuffActDestination, destination};

        assert_eq!(
            destination(878, "EmitterNumChange", &[1]),
            Some(BuffActDestination::StateConsumer)
        );
        assert_eq!(destination(878, "EmitterNumChange", &[0]), None);
        assert_eq!(destination(878, "AttrByShield", &[1]), None);
    }

    #[test]
    fn field_attack_limit_converts_only_bounded_overflow() {
        let limit = || vec![vec![1080, 5, 3000, 5]];
        assert_eq!(limit_attacks(4, limit()), (4, 0));
        assert_eq!(limit_attacks(5, limit()), (5, 0));
        assert_eq!(limit_attacks(6, limit()), (5, 3000));
        assert_eq!(limit_attacks(10, limit()), (5, 15000));
        assert_eq!(limit_attacks(15, limit()), (5, 15000));
        assert_eq!(limit_attacks(6, vec![]), (6, 0));
    }

    fn split_feature() -> ActiveBuffFeature {
        ActiveBuffFeature {
            owner_uid: emitter::UID,
            source_uid: 1,
            buff_uid: 1,
            buff_id: 30480241,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "AttackNumSplitEmitterNum".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: "892#1#2#1".to_owned(),
            values: vec![892, 1, 2, 1],
        }
    }

    #[test]
    fn attack_num_split_feature_only_splits_on_its_configured_cadence() {
        let feature = split_feature();

        assert_eq!(attack_count_delta(&feature), 0);
        assert_eq!(split_count_on_attack(&feature, 1), 1);
        assert_eq!(split_count_on_attack(&feature, 2), 0);
        assert_eq!(split_count_on_attack(&feature, 3), 1);
    }

    #[test]
    fn added_split_targets_apply_only_when_the_cadence_splits() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(emitter::UID),
                    current_hp: Some(1),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(30480241),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(30480211),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut buffs = BuffManager::default();
        buffs.seed(&fight);
        let mut hp = HpManager::default();
        hp.seed(&fight);

        assert_eq!(split_count(&buffs, &hp, 1), 2);
        assert_eq!(split_count(&buffs, &hp, 2), 0);
        assert_eq!(split_count(&buffs, &hp, 3), 2);
    }

    #[test]
    fn split_targets_use_the_default_final_damage_penalty() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(emitter::UID),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        buff_id: Some(30480241),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut buffs = BuffManager::default();
        buffs.seed(&fight);
        let mut hp = HpManager::default();
        hp.seed(&fight);

        assert_eq!(split_final_damage_delta(&buffs, &hp, 1), -700);
        assert_eq!(split_final_damage_delta(&buffs, &hp, 0), 0);
    }
}
