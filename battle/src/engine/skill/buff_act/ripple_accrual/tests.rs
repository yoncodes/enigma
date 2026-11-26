use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    event::payload::{BattleEvent, HealCast},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
        hp::{HpCommand, HpHeal, HpHealKind},
    },
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        effect::SkillEffectCatalog,
        rule::{
            CommandOrigin, DefinitionKey, RuleDomain,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn ripple_accrues_overheal_and_keeps_the_qualifying_cast() {
    crate::test_support::init_config();
    let unit = |uid, team, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        position: Some(position),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    skill_group2: vec![31570124],
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31572002),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..unit(10, 1, 1)
                },
                unit(11, 1, 2),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![unit(-1, 2, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut determinism = RoundDeterminism::default();
    let mut run = |managers: &mut BattleManagers, ops| {
        drain::run_command_group(
            managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            ops,
        )
        .unwrap()
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    run(
        &mut managers,
        vec![RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 31572000,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        )))],
    );
    let uid = managers.buff.buff_id_uid(10, 31572000).unwrap();
    assert_eq!(
        managers.buff.snapshot(10, uid).unwrap().act_info[0].param,
        [1_500]
    );

    let healing = run(
        &mut managers,
        vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
            HpHeal {
                origin,
                source_uid: 10,
                target_uid: 11,
                amount: 802,
                config_effect: 0,
                kind: HpHealKind::Normal,
            },
        )))],
    );
    assert!(
        !healing
            .events
            .iter()
            .any(|event| matches!(event, BattleEvent::HpHealed { .. }))
    );
    let ripple = |managers: &BattleManagers| {
        let uid = managers.buff.buff_id_uid(11, 31572001).unwrap();
        managers.buff.snapshot(11, uid).unwrap().act_info[0].param[0]
    };
    assert_eq!(ripple(&managers), 160);

    managers.hp.lose(11, 50, 0);
    let mut cast = SkillInvocation::from(SkillRequest {
        source_uid: 10,
        skill_id: 31570124,
    });
    cast.mode = SkillExecutionMode::Active;
    let action = run(&mut managers, vec![RuleOp::Skill(cast)]);
    assert!(action.events.iter().any(|event| matches!(
        event,
        BattleEvent::HpHealed {
            target_uid: 11,
            amount: 50,
            ..
        }
    )));
    let heals = action
        .events
        .iter()
        .filter_map(|event| match event {
            BattleEvent::HealResolved(heal) if heal.target_uid == 11 => Some(*heal),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [direct, reaction] = heals.as_slice() else {
        panic!("expected direct and follow-up heals");
    };
    assert_eq!(direct.cast, Some(HealCast::Direct(31570124)));
    assert_eq!(reaction.cast, Some(HealCast::Reaction(31570124)));
    assert_eq!(
        ripple(&managers),
        (160 + direct.amount * 400 / 1_000 + reaction.amount * 400 / 1_000).min(1_500)
    );
    assert_eq!(managers.hp.current(11), 10_000);
}
