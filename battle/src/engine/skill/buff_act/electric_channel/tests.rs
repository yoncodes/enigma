use super::*;
use crate::engine::{
    event::subscription::SubscriptionKey,
    manager::field::{FieldCommand, FieldOperation},
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillTarget,
        buff_act::{self, BuffActRuleOp, registry},
        condition::extra::ExtraSkillKind,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::TargetPool,
    },
};

#[test]
fn continuing_channel_selects_a_cast_from_its_teams_current_field() {
    crate::test_support::init_config();
    let key = DefinitionKey::new(1039, "UseSkillByElectricLvContinueChannel");
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 20,
        buff_uid: 43,
        buff_id: 31320114,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(EventKind::RoundEnd, key),
        act_type: key.type_name.to_owned(),
        effect_time: 302,
        effect_condition: 0,
        args: vec![31320150, 31320151, 31320152, 31320153],
        raw: "1039#31320150#31320151#31320152#31320153".to_owned(),
    };
    assert_eq!(
        registry::runtime_event(key.opcode, key.type_name, subscriber.effect_time),
        Some(EventKind::RoundEnd),
    );

    let managers = BattleManagers::default();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let pool = TargetPool::from_fight(&sonettobuf::Fight::default());
    let expect_skill = |managers: &BattleManagers, skill_id| {
        let ops = buff_act::try_rule_ops(
            managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            &subscriber,
            Some(&BattleEvent::Kind(EventKind::RoundEnd)),
        )
        .unwrap()
        .unwrap();
        assert!(matches!(
            ops.as_slice(),
            [BuffActRuleOp { op: RuleOp::Skill(invocation), .. }]
                if invocation.plan == (SkillRequest { source_uid: 10, skill_id })
                    && invocation.mode == SkillExecutionMode::Active
                    && invocation.target == SkillTarget::Configured
                    && invocation.extra_skill_kind == Some(ExtraSkillKind::ExtraAction)
        ));
    };
    expect_skill(&managers, 31320150);
    let thresholds = managers.catalog().magic_circle_thresholds();
    for threshold in thresholds.iter().filter(|threshold| threshold.level > 0) {
        let mut managers = BattleManagers::default();
        managers
            .field
            .execute_command(FieldCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key,
                },
                team: 1,
                operation: FieldOperation::DeployReplacement {
                    definition: threshold.definition,
                    owner_uid: 20,
                    create_uid: 20,
                    level: threshold.level,
                },
            })
            .unwrap();
        expect_skill(&managers, subscriber.args[threshold.level as usize]);
    }
}
