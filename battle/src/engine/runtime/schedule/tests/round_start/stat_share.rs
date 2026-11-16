use super::*;
use crate::engine::event::payload::BuffExpiry;
use sonettobuf::{BuffActInfo, effect_type_enum::EffectType};

#[test]
fn stat_share_expiry_settles_each_holder_before_the_next_tick() {
    init_config();
    let holder = |uid, current_hp| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(1_800),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(uid + 10),
            from_uid: Some(10),
            buff_id: Some(31460131),
            duration: Some(1),
            act_info: vec![BuffActInfo {
                act_id: Some(1131),
                str_param: Some("101#800".into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    buffs: [(18, 31460111), (19, 31440116)]
                        .map(|(uid, buff_id)| BuffInfo {
                            uid: Some(uid),
                            buff_id: Some(buff_id),
                            from_uid: Some(10),
                            duration: Some(1),
                            ..Default::default()
                        })
                        .to_vec(),
                    ..Default::default()
                },
                holder(11, 1_750),
                holder(12, 650),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let tick = run_round_start_duration(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 4,
            ..Default::default()
        },
        &[(10, 18), (10, 19), (11, 21), (12, 22)],
    )
    .unwrap();
    assert_eq!(managers.hp.max(11), 1_000);
    assert_eq!(managers.hp.current(11), 1_000);
    assert_eq!(managers.hp.max(12), 1_000);
    assert_eq!(managers.hp.current(12), 650);
    assert_eq!(managers.hp.current(10), 10_000);
    assert!(!managers.buff.has_buff_id(11, 31460131));
    assert!(!managers.buff.has_buff_id(12, 31460131));
    let Some(RuleOutcome::BuffBatch(unrelated)) = tick.outcomes.first() else {
        panic!("unrelated timers must commit together");
    };
    assert_eq!(
        unrelated
            .iter()
            .filter_map(|changes| changes.change.removed.first()?.buff.uid)
            .collect::<Vec<_>>(),
        [18, 19]
    );
    assert_eq!(
        tick.events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::BuffRemoved(change) if change.target_uid != 10 =>
                    Some((change.target_uid, change.expiry)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (11, BuffExpiry::Duration(103)),
            (12, BuffExpiry::Duration(103))
        ],
    );
    let steps = crate::engine::packet::timeline::project_for_version(&tick.frames, 7).unwrap();
    let [step] = steps.as_slice() else {
        panic!("expiry must share its round-start step")
    };
    let first_holder = step
        .act_effect
        .iter()
        .position(|effect| effect.target_id == Some(11))
        .unwrap();
    let [
        delete_first,
        max_first,
        check_first,
        delete_second,
        max_second,
        check_second,
    ] = &step.act_effect[first_holder..]
    else {
        panic!("expected two expiry transactions: {:?}", step.act_effect);
    };
    for (uid, deleted, maxima) in [
        (11, delete_first, [max_first, check_first]),
        (12, delete_second, [max_second, check_second]),
    ] {
        assert_eq!(deleted.effect_type, Some(EffectType::Buffdel as i32));
        assert_eq!(deleted.target_id, Some(uid));
        for max in maxima {
            assert_eq!(max.effect_type, Some(EffectType::Maxhpchange as i32));
            assert_eq!(max.target_id, Some(uid));
            assert_eq!(max.effect_num, Some(1_000));
            assert_eq!(max.buff_act_id, Some(1131));
        }
    }
}
