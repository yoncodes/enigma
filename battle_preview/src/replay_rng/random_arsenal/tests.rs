use super::*;
use battle::tooling::scan::effect_catalog;
use sonettobuf::{ActEffect, BuffInfo};

#[test]
fn arsenal_import_requires_a_complete_ordered_witness_from_one_cast() {
    crate::init_test_config();
    let complete = |id, uid| {
        (1..=4).map(move |count| ActEffect {
            target_id: Some(10),
            effect_type: Some(if count == 1 {
                EffectType::Buffadd
            } else {
                EffectType::Buffupdate
            } as i32),
            buff: Some(BuffInfo {
                uid: Some(uid),
                buff_id: Some(id),
                from_uid: Some(10),
                count: Some(count),
                ..Default::default()
            }),
            ..Default::default()
        })
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_type: Some(ActType::Skill as i32),
            act_id: Some(30920144),
            from_id: Some(10),
            to_id: Some(-1),
            act_effect: complete(30920153, 100)
                .chain(complete(30920155, 104))
                .collect(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let catalog = effect_catalog::global();
    let candidates = choices(catalog, &round);
    let [choice] = candidates.as_slice() else {
        panic!("expected one complete Arsenal witness");
    };
    assert_eq!(choice.source_uid, 10);
    assert_eq!(choice.target_uid, 10);
    assert_eq!(choice.rule.effect_id, 30920144);
    assert_eq!(choice.buff_ids, [30920153, 30920155]);

    let mut missing = round.clone();
    missing.fight_step[0].act_effect.pop();
    assert!(choices(catalog, &missing).is_empty());

    let mut another_uid = round.clone();
    another_uid.fight_step[0].act_effect[2]
        .buff
        .as_mut()
        .unwrap()
        .uid = Some(101);
    assert!(choices(catalog, &another_uid).is_empty());

    let mut ambiguous = round;
    ambiguous.fight_step.push(ambiguous.fight_step[0].clone());
    assert!(choices(catalog, &ambiguous).is_empty());
}
