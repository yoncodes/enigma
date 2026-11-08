use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::{action::SkillTrigger, rule::output::RuleOp},
};

fn subscriber(args: Vec<i32>) -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 11,
        source_uid: 11,
        buff_uid: 1,
        buff_id: 31140144,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::AllyAction,
            crate::engine::skill::rule::DefinitionKey::new(927, "AddBuffByOtherExSkill"),
        ),
        act_type: "AddBuffByOtherExSkill".into(),
        effect_time: 212,
        effect_condition: 0,
        args,
        raw: "927".into(),
    }
}

fn granted(op: RuleOp) -> BuffGrant {
    let RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) = op else {
        panic!("expected a buff grant");
    };
    grant
}

#[test]
fn other_ultimate_grants_one_narrative_per_spent_moxie() {
    crate::test_support::init_config();
    let catalog = crate::engine::skill::effect::catalog::global();
    let subscriber = subscriber(vec![1, 31140113, 31140114]);

    let single =
        granted(rule_op(catalog, &subscriber, 22, 30630183, 1, 5, SkillTrigger::None).unwrap());
    assert_eq!((single.buff_id, single.amount), (31140113, Some(5)));

    let mass =
        granted(rule_op(catalog, &subscriber, 22, 31020131, 2, 5, SkillTrigger::None).unwrap());
    assert_eq!((mass.buff_id, mass.amount), (31140114, Some(5)));
    assert!(rule_op(catalog, &subscriber, 11, 30630183, 1, 5, SkillTrigger::None,).is_none());
}

#[test]
fn configured_direct_ultimate_adds_its_bonus_per_spent_moxie() {
    crate::test_support::init_config();
    let catalog = crate::engine::skill::effect::catalog::global();
    let subscriber = subscriber(vec![1, 31140113, 31140114, 311401511, 1]);

    let grant = granted(
        rule_op(
            catalog,
            &subscriber,
            22,
            30630183,
            1,
            5,
            SkillTrigger::Explicit(311401511),
        )
        .unwrap(),
    );

    assert_eq!(grant.amount, Some(10));

    let base_grant = granted(
        rule_op(
            catalog,
            &subscriber,
            22,
            30630183,
            1,
            5,
            SkillTrigger::Explicit(30630183),
        )
        .unwrap(),
    );

    assert_eq!(base_grant.amount, Some(5));
}
