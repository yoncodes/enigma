use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

fn fight() -> Fight {
    let entity = |uid, attack| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut carrier = entity(2, 2_000);
    carrier.buffs.push(BuffInfo {
        uid: Some(10),
        buff_id: Some(31_400_003),
        from_uid: Some(2),
        act_info: vec![BuffActInfo {
            act_id: Some(1147),
            param: vec![1_696, 10_605],
            str_param: Some(String::new()),
        }],
        ..Default::default()
    });
    Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 500), carrier],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(3, 400)],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn foundation_counter_reads_fill_only_for_another_allied_riposte() {
    crate::test_support::init_config();
    let fight = fight();
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    for (actor, kind) in [
        (1, ExtraSkillKind::FollowUp),
        (2, ExtraSkillKind::Riposte),
        (3, ExtraSkillKind::Riposte),
    ] {
        assert!(
            RiposteDamage::for_action(&managers, &pool, actor, kind.id())
                .unwrap()
                .is_none()
        );
    }

    let plan = RiposteDamage::for_action(&managers, &pool, 1, ExtraSkillKind::Riposte.id())
        .unwrap()
        .unwrap();
    assert_eq!(plan.source_uid, 2);
    assert_eq!(plan.rate, 1_159);
    let again = RiposteDamage::for_action(&managers, &pool, 1, ExtraSkillKind::Riposte.id())
        .unwrap()
        .unwrap();
    assert_eq!(again.rate, 1_159);
}

#[test]
fn foundation_riposte_rejects_malformed_counter_state() {
    crate::test_support::init_config();
    let mut fight = fight();
    fight.attacker.as_mut().unwrap().entitys[1].buffs[0].act_info[0].param[0] = 10_606;
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    assert!(matches!(
        RiposteDamage::for_action(&managers, &pool, 1, ExtraSkillKind::Riposte.id()),
        Err(CounterError::InvalidState)
    ));
}

#[test]
fn foundation_riposte_uses_credited_attack_and_target_reduction() {
    crate::test_support::init_config();
    let fight = fight();
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let planner = RiposteDamage::for_action(&managers, &pool, 1, ExtraSkillKind::Riposte.id())
        .unwrap()
        .unwrap();
    let runtime = damage::DamageRuntime {
        fight_version: 7,
        pool: &pool,
        attributes: &managers.attribute,
        buffs: &managers.buff,
        target_buffs: &managers.buff,
        hp: &managers.hp,
        fields: None,
        emitter: None,
        team_inspiration: 0,
    };
    let mut determinism = RoundDeterminism::default();
    let skill_id = 30941172;
    determinism.enqueue_additional_crits(skill_id, 1, 3, [false, false]);
    let HpCommand::Damage(main) = damage::resolve_avoided_attack_command(1, 3, planner.origin)
    else {
        panic!("expected damage command");
    };
    let hit = |attributes: &[(AttrId, i32)], determinism: &mut RoundDeterminism| {
        let Some(HpCommand::Damage(hit)) =
            planner.hit(main, skill_id, attributes, &managers, runtime, determinism)
        else {
            panic!("expected additional damage");
        };
        assert_eq!((hit.source_uid, hit.target_uid), (2, 3));
        assert_eq!(
            hit.hurt.damage_from,
            crate::engine::manager::hp::HurtDamageFromType::Additional
        );
        hit.amount
    };
    assert_eq!(hit(&[], &mut determinism), 2318);
    assert_eq!(
        hit(&[(AttrId::DmgTakenReduction, 250)], &mut determinism),
        1738
    );
}
