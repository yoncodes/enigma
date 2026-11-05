use super::*;
use sonettobuf::{ActEffect, FightHurtInfo, FightStep};

#[test]
fn observations_use_the_damage_source_over_the_outer_actor() {
    let damage = |source_uid| ActEffect {
        effect_type: Some(EffectType::Additionaldamage as i32),
        target_id: Some(30),
        effect_num: Some(100),
        hurt_info: Some(FightHurtInfo {
            from_uid: Some(source_uid),
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_id: Some(40),
            from_id: Some(10),
            act_effect: vec![damage(10), damage(20)],
            ..Default::default()
        }],
        ..Default::default()
    };
    let sources = observations(&round)
        .into_iter()
        .map(|(identity, _)| identity.source_uid)
        .collect::<Vec<_>>();
    assert_eq!(sources, vec![10, 20]);
}

#[test]
fn final_recipient_comparison_retains_missing_and_duplicate_hits() {
    let round = |targets: &[i64]| FightRound {
        fight_step: vec![FightStep {
            act_id: Some(40),
            from_id: Some(10),
            act_effect: targets
                .iter()
                .map(|uid| ActEffect {
                    effect_type: Some(EffectType::Crit as i32),
                    target_id: Some(*uid),
                    effect_num: Some(100),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let captured = round(&[-1, -2, -3]);
    assert!(lines(&captured, &captured, false).is_empty());
    let verbose = lines(&captured, &captured, true);
    assert_eq!(verbose.len(), 3);
    assert!(
        verbose
            .iter()
            .all(|line| line.contains("earliest_observable_divergence=none"))
    );
    let missing = lines(&round(&[-1, -2]), &captured, false);
    assert_eq!(missing.len(), 1);
    assert!(missing[0].contains("target_uid: -3"));
    assert!(missing[0].contains("generated=missing"));
    let duplicate = lines(&round(&[-1, -2, -3, -3]), &captured, false);
    assert_eq!(duplicate.len(), 1);
    assert!(duplicate[0].contains("target_uid: -3"));
    assert!(duplicate[0].contains("captured=missing"));
    let mut changed = captured.clone();
    changed.fight_step[0].act_effect[0].effect_num = Some(110);
    let amount = lines(&changed, &captured, false);
    assert_eq!(amount.len(), 1);
    assert!(amount[0].contains("delta=10"));
    changed = captured.clone();
    changed.fight_step[0].act_effect[0].hurt_info = Some(FightHurtInfo {
        career_restraint: Some(true),
        ..Default::default()
    });
    let restraint = lines(&changed, &captured, false);
    assert_eq!(restraint.len(), 1);
    assert!(restraint[0].contains("restrained=true/false"));
    assert!(restraint[0].contains("earliest_observable_divergence=final_effect"));
}

#[test]
fn full_round_comparison_keeps_non_damage_targets_and_order() {
    let round = |targets: &[i64]| {
        serde_json::to_value(FightRound {
            fight_step: targets
                .iter()
                .map(|uid| FightStep {
                    act_id: Some(40),
                    from_id: Some(10),
                    to_id: Some(*uid),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
        .unwrap()
    };
    let (generated, captured) =
        battle_preview::canonical_comparison(round(&[-1, -2]), round(&[-2, -1]));
    assert!(battle_preview::first_diff_path(&generated, &captured, "").is_some());
}
