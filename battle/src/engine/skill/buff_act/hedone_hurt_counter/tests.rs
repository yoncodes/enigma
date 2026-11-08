use crate::engine::{
    event::subscription::SubscriptionKey,
    manager::hp::HurtDamageFromType,
    runtime::{
        determinism::RoundDeterminism,
        drain,
        executor::RuleOutcome,
        record::{FrameItem, FrameOwner, SetupSide},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam};

use super::*;

fn setup(scarred: bool) -> (BattleManagers, TargetPool, BuffActSubscriber) {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-4),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(1070),
                        buff_id: Some(31560102),
                        from_uid: Some(-4),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1158),
                            param: vec![0],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(100),
                buffs: scarred
                    .then(|| BuffInfo {
                        uid: Some(2000),
                        buff_id: Some(31560121),
                        from_uid: Some(-4),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: -4,
        source_uid: -4,
        buff_uid: 1070,
        buff_id: 31560102,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::TargetAttacked,
            DefinitionKey::new(1158, "HedoneHurtCounter"),
        ),
        act_type: "HedoneHurtCounter".into(),
        effect_time: 0,
        effect_condition: 0,
        args: vec![100, 31560121, 100, 4],
        raw: "1158#100#31560121#100#4".into(),
    };
    (managers, pool, subscriber)
}

fn hit(source_uid: i64, damage: i32) -> BattleEvent {
    BattleEvent::Hit(crate::engine::event::payload::HitEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        },
        source_uid,
        target_uid: 1,
        skill_id: 1,
        amount: damage,
        resolved_damage: damage,
        shield_absorbed: 0,
        career_restraint: false,
        damage_from: HurtDamageFromType::Skill,
        share_count: 0,
        assassinate: false,
        ignore_riposte: false,
    })
}

#[test]
fn initializes_the_counter_at_enter_battle_static() {
    let (mut managers, pool, _) = setup(false);
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1158, "HedoneHurtCounter"),
    };
    let plan = managers
        .plan_buff(BuffCommand::SetInternalState(BuffSetState {
            origin,
            target_uid: -4,
            buff_uid: 1070,
            ex_info: None,
            params: None,
            act_info: Some(Vec::new()),
        }))
        .unwrap();
    managers.commit_buff(plan);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.act_id() == Some(1158))
        .unwrap();
    let result = drain::run_setup_stage(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        SetupStage::EnterBattleStatic,
        0,
    )
    .unwrap();

    assert_eq!(
        managers
            .buff
            .snapshot(-4, 1070)
            .unwrap()
            .act_info
            .as_slice(),
        &[BuffActInfo {
            act_id: Some(1158),
            param: vec![0],
            str_param: Some(String::new()),
        }]
    );
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::BuffActInfoMarker(marker)
            if marker.buff_uid == 1070 && marker.act_id == 1158 && marker.params == [0]
    )));
    let [side] = result.frames.as_slice() else {
        panic!("expected one setup-side frame");
    };
    assert_eq!(side.owner, FrameOwner::SetupSide(SetupSide::Attacker));
    let [FrameItem::Child(counter)] = side.items.as_slice() else {
        panic!("expected the counter directly under its side");
    };
    assert!(matches!(
        counter.owner,
        FrameOwner::BuffAct {
            owner_uid: -4,
            buff_uid: 1070,
            buff_id: 31560102,
            key,
            ..
        } if key == DefinitionKey::new(1158, "HedoneHurtCounter")
    ));
    assert!(
        setup_rule_ops(&managers, &feature, SetupStage::EnterBattleStatic)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn records_each_allied_hit_with_capture_rounding() {
    let (mut managers, pool, subscriber) = setup(false);
    for damage in [22_184, 3_013_646, 31_164] {
        let ops = rule_ops(&managers, &pool, &subscriber, &hit(-1, damage)).unwrap();
        let [RuleOp::Command(BattleCommand::HedoneCounter(command))] = ops.as_slice() else {
            panic!("expected one counter command");
        };
        managers.hedone_counter.execute(*command).unwrap();
    }
    assert_eq!(managers.hedone_counter.value(1070, 0), Ok(306_698));
}

#[test]
fn ignores_opposing_hits() {
    let (managers, pool, subscriber) = setup(false);
    assert_eq!(
        rule_ops(&managers, &pool, &subscriber, &hit(1, 1_000)).unwrap(),
        Vec::new()
    );
}

#[test]
fn adds_the_configured_molten_scar_rate() {
    let (managers, pool, subscriber) = setup(true);
    let ops = rule_ops(&managers, &pool, &subscriber, &hit(-1, 8_709)).unwrap();
    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::HedoneCounter(
            HedoneCounterCommand::Record(HedoneCounterRecord { amount: 1_740, .. })
        ))]
    ));
}

#[test]
fn requires_exact_positive_grammar() {
    assert!(supports(&[100, 31560121, 100, 4]));
    assert!(!supports(&[100, 31560121, 100]));
    assert!(!supports(&[0, 31560121, 100, 4]));
    assert!(!supports(&[100, 0, 100, 4]));
    assert!(!supports(&[100, 31560121, -1, 4]));
    assert!(!supports(&[100, 31560121, 100, 0]));
}
