use super::*;
use crate::engine::event::payload::BattleEvent;

#[test]
fn lost_life_attack_uses_all_characters_and_the_configured_hp_basis() {
    crate::test_support::init_config();
    assert!(config::get().skill_buff.get(31200174).is_some());
    let entity = |uid, current_hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 10_000), entity(2, 5_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(109390001),
                    from_uid: Some(-1),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..entity(-1, 8_000)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let configured = BuffManager::configured_features(31200174);
    assert!(
        configured.iter().any(|feature| {
            let mut feature = feature.clone();
            feature.owner_uid = 1;
            is_kind(&feature, BuffActKind::AttrOnlyCalDamageReplaceAttr)
                && crate::engine::skill::buff_act::attack_replacement(&feature, &managers.hp)
                    == Some(2_000)
        }),
        "configured={configured:?}"
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();
    let losses = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 1,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::from_spec(
            BehaviorSpec::new(60212, "LostAllLifeByAttr"),
            vec![AttrId::CurrentHp as i32, 200, AttrId::Hp as i32, 200],
            Vec::new(),
        ),
    )
    .unwrap();
    let amounts = losses
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))) => Some(loss.amount),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(amounts, vec![2_000, 1_000, 1_600]);

    let damage = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 1,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::from_spec(
            BehaviorSpec::new(60216, "DamageRealLostLife"),
            vec![31200174, 3, 500],
            Vec::new(),
        ),
    )
    .unwrap();
    assert!(matches!(
        damage.as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(
            HpDamage {
                amount: 2_100,
                target_uid: -1,
                ..
            }
        )))]
    ));
}

#[test]
fn configured_hp_loss_floor_uses_only_supported_thresholds() {
    crate::test_support::init_config();
    let entity = |uid, buff_id| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(2_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            buff_id: Some(buff_id),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 31200145), entity(2, 31201145), entity(3, 7280005)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(30006, "LostLife"),
        vec![1, AttrId::CurrentHp as i32, 1_000],
        Vec::new(),
    );

    assert_eq!(managers.buff.lost_life_floor_permille(1), 150);
    assert_eq!(
        loss_command(&managers, &pool, 1, &behavior, None).amount,
        500
    );
    assert_eq!(managers.buff.lost_life_floor_permille(2), 160);
    assert_eq!(
        loss_command(&managers, &pool, 2, &behavior, None).amount,
        400
    );
    assert_eq!(managers.buff.lost_life_floor_permille(3), 0);
    assert_eq!(
        loss_command(&managers, &pool, 3, &behavior, None).amount,
        2_000
    );

    let break_loss = ParsedBehavior::from_spec(
        BehaviorSpec::new(30005, "LostLife"),
        vec![0, AttrId::Hp.id(), 1_000],
        Vec::new(),
    );
    let broken = BattleEvent::ToughnessBroken {
        source_uid: -1,
        target_uid: 1,
        skill_id: 10,
        target_defeated: false,
    };
    assert_eq!(
        loss_command(&managers, &pool, 1, &break_loss, Some(&broken)).amount,
        500
    );
}

fn loss_command(
    managers: &BattleManagers,
    pool: &TargetPool,
    target_uid: i64,
    behavior: &ParsedBehavior,
    event: Option<&BattleEvent>,
) -> HpLoss {
    let commands = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: target_uid,
            source_team: if pool.source_is_attacker(target_uid) {
                1
            } else {
                2
            },
            target_uid,
            active_skill_id: 20,
            transfer_count: 1,
            event,
            managers,
            pool,
            determinism: &mut Default::default(),
            modifiers: &mut Default::default(),
            target: &mut TargetContext {
                active_skill_id: 20,
                ..Default::default()
            },
        },
        behavior,
    )
    .unwrap();
    let [RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(command)))] = commands.as_slice() else {
        panic!("expected one lost-life command");
    };
    *command
}

#[test]
fn guard_break_loss_preserves_amount_and_credit_through_settlement() {
    init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 1, 1, 100, 0)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                current_hp: Some(50),
                ..entity(-1, 2, 1, 100, 0)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let broken = BattleEvent::ToughnessBroken {
        source_uid: 1,
        target_uid: -1,
        skill_id: 10,
        target_defeated: false,
    };
    for (opcode, rate, event, skill_id, requested, applied) in [
        (30005, 100, Some(&broken), 10, 100, 50),
        (60310, 100, Some(&broken), 10, 50, 50),
        (30006, 100, Some(&broken), 20, 50, 50),
        (30005, 100, None, 20, 50, 50),
    ] {
        let mut managers = BattleManagers::seeded(&fight);
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(opcode, "LostLife"),
            vec![0, AttrId::Hp.id(), rate],
            Vec::new(),
        );
        let command = loss_command(&managers, &pool, -1, &behavior, event);
        assert_eq!(command.amount, requested);
        let hp = managers
            .execute_hp(HpCommand::Lose(command))
            .unwrap()
            .hp
            .unwrap();
        let hurt = hp.hurt.unwrap();
        assert_eq!(managers.hp.current(-1), 50 - applied);
        assert_eq!((hp.delta, hurt.reduce_hp), (-applied, -applied));
        assert_eq!(
            (hp.display_amount, hurt.display_amount),
            (Some(requested), Some(requested))
        );
        assert_eq!(
            (hurt.from_uid, hurt.effect_id, hurt.skill_id),
            (-1, 20, skill_id)
        );
    }
}
