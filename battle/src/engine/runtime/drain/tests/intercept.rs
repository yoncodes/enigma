use super::*;
use crate::engine::manager::hp::{
    DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData,
};

fn queued(op: RuleOp) -> QueuedOp {
    QueuedOp {
        op,
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: None,
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber_owner_uid: None,
    }
}

fn share_hurt_team(stacks: i32) -> Fight {
    let entity = |uid: i64| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(10_000),
        team_type: Some(1),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        buffs: (uid == 10)
            .then(|| BuffInfo {
                uid: Some(50),
                buff_id: Some(31090121),
                from_uid: Some(13),
                count: Some(stacks),
                ..Default::default()
            })
            .into_iter()
            .collect(),
        ..Default::default()
    };
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10, 11, 12, 13].into_iter().map(entity).collect(),
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(10_000),
                team_type: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn hit_on_holder() -> HpCommand {
    HpCommand::Damage(HpDamage {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        },
        source_uid: -1,
        target_uid: 10,
        amount: 2102,
        config_effect: -1,
        effect_kind: DamageEffectKind::Normal,
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: -1,
            is_crit: false,
            career_restraint: false,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 0,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Damage as i32,
            display_amount: None,
        },
    })
}

fn drain(fight: &Fight, op: RuleOp) -> (BattleManagers, DrainResult) {
    let pool = TargetPool::from_fight(fight);
    let mut managers = BattleManagers::seeded(fight);
    let mut queue = VecDeque::from([queued(op)]);
    let result = drain_queue(
        &mut managers,
        &pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
    )
    .unwrap();
    (managers, result)
}

#[test]
fn registered_hp_intercept_commits_the_split_in_order_and_keeps_the_hit_shape() {
    crate::test_support::init_config();
    let fight = share_hurt_team(3);

    let (managers, result) = drain(&fight, RuleOp::Command(BattleCommand::Hp(hit_on_holder())));

    let shapes = result
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            RuleOutcome::Buff(_) => Some("buff"),
            RuleOutcome::HpBatch(_) => Some("shares"),
            RuleOutcome::Hp(_) => Some("hit"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(shapes, vec!["buff", "shares", "hit"]);
    for uid in [10, 11, 12, 13] {
        assert_eq!(managers.hp.current(uid), 10_000 - 525);
    }
}

#[test]
fn a_spent_holder_takes_its_next_hit_unsplit() {
    crate::test_support::init_config();
    let fight = share_hurt_team(1);

    let (managers, _) = drain(
        &fight,
        RuleOp::Command(BattleCommand::HpBatch(vec![
            hit_on_holder(),
            hit_on_holder(),
        ])),
    );

    assert!(!managers.buff.has_buff_id(10, 31090121));
    assert_eq!(managers.hp.current(10), 10_000 - 525 - 2102);
    for uid in [11, 12, 13] {
        assert_eq!(managers.hp.current(uid), 10_000 - 525);
    }
}
