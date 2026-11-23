use battle::tooling::{
    replay::ConfiguredRuleKey,
    scan::{ParsedSkillEffect, effect_catalog},
};
use sonettobuf::{ActEffect, BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn opening_random_buffs_keep_the_configured_producer_and_eligibility() {
    crate::init_test_config();
    let fight = Fight {
        battle_id: Some(120380800),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                passive_skill: vec![30630151],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let round = FightRound {
        fight_step: vec![FightStep {
            act_id: Some(30630151),
            from_id: Some(10),
            act_effect: [30630112, 30630114]
                .map(|buff_id| ActEffect {
                    target_id: Some(11),
                    effect_type: Some(EffectType::Buffadd as i32),
                    buff: Some(BuffInfo {
                        buff_id: Some(buff_id),
                        from_uid: Some(10),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let slot = &effect_catalog::global().get(30630151).unwrap().slots[0];
    let rule = ConfiguredRuleKey::new(
        slot.behavior.effect_id,
        slot.behavior.slot,
        behavior::registry::find(&slot.behavior).unwrap().key,
    );
    let pool = [30630112, 30630113, 30630114, 30630115];
    let mut seeded = crate::opening_determinism(config::configs::get(), &fight, &round).unwrap();
    assert_eq!(seeded.take_random_buff_pool(rule, 20, 11, 2, &pool), None);
    assert_eq!(seeded.take_random_buff_pool(rule, 10, 12, 2, &pool), None);
    assert_eq!(
        seeded.take_random_buff_pool(rule, 10, 11, 2, &pool),
        Some(vec![30630112, 30630114])
    );

    let mut seeded = crate::opening_determinism(config::configs::get(), &fight, &round).unwrap();
    assert_eq!(
        seeded.take_random_buff_pool(rule, 10, 11, 2, &pool[..2]),
        None
    );
    assert_eq!(seeded.take_random_buff_pool(rule, 10, 11, 2, &pool), None);

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 30630151,
        slots: vec![slot.clone(), slot.clone()],
    });
    assert!(choices(&catalog, &round).is_empty());

    let mut one_draw = round.clone();
    one_draw.fight_step[0].act_effect.truncate(1);
    assert!(choices(effect_catalog::global(), &one_draw).is_empty());

    let mut competing = slot.clone();
    competing.behavior.spec =
        battle::tooling::scan::behavior::classify::BehaviorSpec::new(20022, "AddBuffRanTypeId");
    competing.behavior.slot = 3;
    competing.behavior.args[1] = 1;
    competing.behavior.raw_args.clear();
    catalog.insert(ParsedSkillEffect {
        skill_id: 30630151,
        slots: vec![slot.clone(), competing],
    });
    assert!(choices(&catalog, &round).is_empty());
}
