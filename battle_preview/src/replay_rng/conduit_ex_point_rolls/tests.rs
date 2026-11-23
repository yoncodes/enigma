use super::*;
use sonettobuf::{ActEffect, FightExPointInfo};

fn gain(target: i64, amount: i32) -> ActEffect {
    ActEffect {
        target_id: Some(target),
        effect_type: Some(EffectType::Expointchange as i32),
        effect_num: Some(amount),
        ..Default::default()
    }
}

fn nested(step: FightStep) -> ActEffect {
    ActEffect {
        fight_step: Some(step),
        ..Default::default()
    }
}

fn group(grants: &[(i64, i32)]) -> FightStep {
    FightStep {
        act_effect: grants
            .iter()
            .map(|&(target, amount)| {
                nested(FightStep {
                    act_effect: vec![gain(target, amount)],
                    ..Default::default()
                })
            })
            .collect(),
        ..Default::default()
    }
}

fn change_round() -> FightStep {
    FightStep {
        act_effect: vec![ActEffect {
            effect_type: Some(EffectType::Changeround as i32),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn holder(uid: i64) -> FightExPointInfo {
    FightExPointInfo {
        uid: Some(uid),
        ex_point_type: Some(conduit_ex_point_type()),
        ..Default::default()
    }
}

fn rolls(choices: &[ConditionRandomChoice]) -> Vec<(i64, i32)> {
    choices
        .iter()
        .map(|choice| (choice.source_uid, choice.roll))
        .collect()
}

#[test]
fn each_evaluation_point_passes_only_the_observed_gainers() {
    let round = FightRound {
        ex_point_info: vec![holder(1), holder(2)],
        fight_step: vec![
            group(&[(1, 30)]),
            change_round(),
            FightStep::default(),
            change_round(),
            group(&[(2, 50), (1, 25)]),
        ],
        ..Default::default()
    };
    let choices = observed_choices(&[(7, 1022101)], &round, true);
    assert!(
        choices
            .iter()
            .all(|choice| choice.skill_id == 7 && choice.opcode == 1022101)
    );
    assert_eq!(
        rolls(&choices),
        [
            (1, PASS),
            (2, FAIL),
            (1, FAIL),
            (2, FAIL),
            (1, PASS),
            (2, PASS)
        ]
    );
}

#[test]
fn a_capped_character_that_only_banks_still_passed() {
    let mut capped = group(&[(2, 0)]);
    if let Some(child) = capped.act_effect[0].fight_step.as_mut() {
        child.act_effect = vec![nested(FightStep {
            from_id: Some(2),
            act_effect: vec![ActEffect {
                target_id: Some(2),
                effect_type: Some(EffectType::Buffactinfoupdate as i32),
                buff_act_info: Some(sonettobuf::BuffActInfo {
                    act_id: Some(conduit_ex_point_overflow_bank_act()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        })];
    }
    capped.act_effect.extend(group(&[(1, 42)]).act_effect);
    let round = FightRound {
        ex_point_info: vec![holder(1), holder(2)],
        fight_step: vec![capped],
        ..Default::default()
    };
    assert_eq!(
        rolls(&observed_choices(&[(7, 1022101)], &round, true)),
        [(1, PASS), (2, PASS)]
    );
}

#[test]
fn steps_with_a_source_or_other_effects_are_not_roll_groups() {
    let mut sourced = group(&[(1, 50)]);
    if let Some(child) = sourced.act_effect[0].fight_step.as_mut() {
        child.act_id = Some(31450173);
    }
    let mut mixed = group(&[(1, 50)]);
    mixed.act_effect.push(ActEffect {
        effect_type: Some(EffectType::Devicepowerclear as i32),
        ..Default::default()
    });
    let round = FightRound {
        ex_point_info: vec![holder(1)],
        fight_step: vec![sourced, mixed],
        ..Default::default()
    };
    assert_eq!(
        rolls(&observed_choices(&[(7, 1022101)], &round, true)),
        [(1, FAIL)]
    );
}

#[test]
fn a_later_group_does_not_fill_a_round_change_roll() {
    let round = FightRound {
        ex_point_info: vec![holder(1)],
        fight_step: vec![change_round(), FightStep::default(), group(&[(1, 50)])],
        ..Default::default()
    };

    assert_eq!(
        rolls(&observed_choices(&[(7, 1022101)], &round, false)),
        [(1, FAIL)]
    );
}
