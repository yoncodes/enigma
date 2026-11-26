use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        card::CARD_PLAY_ORIGIN,
        ex_point::{ExPointChange, ExPointCommand},
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
fn moxie_field_samples_only_its_lifetime_payments_at_ally_preparation() {
    crate::test_support::init_config();
    let unit = |uid, team, position, moxie| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        position: Some(position),
        current_hp: Some(1_000),
        ex_point: Some(moxie),
        ex_point_max: Some(30),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                unit(10, 1, 1, 30),
                FightEntityInfo {
                    skill_group1: vec![31570111],
                    ..unit(11, 1, 2, 0)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![unit(-1, 2, 1, 10)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut determinism = RoundDeterminism::default();
    let mut run = |managers: &mut BattleManagers, op| {
        drain::run_command_group(
            managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            [op],
        )
        .unwrap()
    };
    let pay = |source_uid, amount: i32| RuleOp::PaySkillAction {
        action: SkillRequest {
            source_uid,
            skill_id: 31571131,
        },
        cost: ExPointCommand::Spend(ExPointChange {
            origin: CARD_PLAY_ORIGIN,
            source_uid,
            target_uid: source_uid,
            delta: -amount,
            config_effect: 0,
            effect_type: 0,
        }),
    };
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 31571007,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let cast = |mode| {
        let mut invocation = SkillInvocation::from(SkillRequest {
            source_uid: 11,
            skill_id: 31570111,
        });
        invocation.mode = mode;
        RuleOp::Skill(invocation)
    };

    run(&mut managers, pay(10, 5));
    run(&mut managers, grant());
    let uid = managers.buff.buff_id_uid(10, 31571007).unwrap();
    let value =
        |managers: &BattleManagers| managers.buff.snapshot(10, uid).unwrap().act_info[0].param[0];
    assert_eq!(value(&managers), 0);
    assert!(managers.buff.has_force_field(10));
    assert_eq!(managers.origin_attribute(11, AttrId::UltimateMight), 400);

    run(&mut managers, pay(-1, 5));
    run(&mut managers, pay(10, 5));
    let state = managers.buff.moxie_field(10, uid).unwrap();
    assert_eq!((state.credited, state.sampled), (5, 0));
    assert_eq!(managers.origin_attribute(11, AttrId::UltimateMight), 400);
    let mut observed = managers.buff.snapshot(10, uid).unwrap();
    observed.duration = Some(1);
    assert!(managers.buff.resync_observed_present(10, observed).unwrap());
    assert_eq!(managers.buff.moxie_field(10, uid), Some(state));
    run(&mut managers, grant());
    assert_eq!(managers.buff.buff_id_uid(10, 31571007), Some(uid));
    assert_eq!(value(&managers), 0);
    run(&mut managers, cast(SkillExecutionMode::Active));
    assert_eq!(value(&managers), 5);
    assert_eq!(managers.origin_attribute(11, AttrId::UltimateMight), 500);
    assert_eq!(managers.origin_attribute(-1, AttrId::UltimateMight), 0);

    run(&mut managers, pay(10, 13));
    run(&mut managers, cast(SkillExecutionMode::Nested));
    assert_eq!(value(&managers), 5);
    run(&mut managers, cast(SkillExecutionMode::NestedExtra));
    assert_eq!(value(&managers), 18);
    assert_eq!(managers.origin_attribute(11, AttrId::UltimateMight), 760);

    run(&mut managers, pay(10, 5));
    run(&mut managers, grant());
    assert_eq!(value(&managers), 18);
    run(&mut managers, cast(SkillExecutionMode::Active));
    assert_eq!(value(&managers), 20);
    assert_eq!(managers.origin_attribute(11, AttrId::UltimateMight), 800);

    run(
        &mut managers,
        RuleOp::Command(BattleCommand::Buff(BuffCommand::RemoveAfterTrigger(
            BuffRemove {
                origin,
                target_uid: 10,
                selector: BuffRemoveSelector::Uid(uid),
            },
        ))),
    );
    assert_eq!(managers.buff.moxie_field(10, uid), None);
    assert_eq!(managers.origin_attribute(11, AttrId::UltimateMight), 0);
}
