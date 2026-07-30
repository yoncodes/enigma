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
    -if configured_reduction == 0 {
        DEFAULT_SPLIT_DAMAGE_REDUCTION
    } else {
        configured_reduction
    }
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
