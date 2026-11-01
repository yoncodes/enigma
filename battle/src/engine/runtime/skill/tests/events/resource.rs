use super::*;

fn catalog(condition: ParsedCondition) -> SkillEffectCatalog {
    let mut slot = SkillEffectSlot::new(
        ParsedBehavior::new(50012, "DirectUseSkillNoAct", vec![200, 1]),
        TargetRequest {
            code: 101,
            raw: Vec::new(),
        },
    );
    slot.conditions = vec![condition];
    slot.compiled_route =
        crate::engine::skill::rule::route::ConditionRoute::compile(&slot.conditions);

    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![slot],
    });
    catalog
}

fn condition(opcode: i32, type_name: &str, args: &[&str]) -> ParsedCondition {
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    ParsedCondition {
        opcode,
        type_name: type_name.to_owned(),
        kind: crate::engine::skill::condition::registry::parse(opcode, type_name, &args).unwrap(),
        raw_args: args,
    }
}

fn eureka_event(delta: i32) -> BattleEvent {
    let before = if delta < 0 { 2 } else { 1 };
    BattleEvent::EurekaChanged(crate::engine::event::payload::EurekaChangeEvent {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "SpendEureka"),
        },
        source_uid: 20,
        target_uid: 20,
        power_id: 1,
        before,
        requested_delta: delta,
        applied_delta: delta,
        after: before + delta,
        overflow: 0,
    })
}

fn child_target(
    condition: ParsedCondition,
    key: DefinitionKey,
    context: TargetContext,
    delta: i32,
) -> i64 {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = catalog(condition);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.condition_key = Some(key);

    let children = child_skills(
        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            context,
            &SkillOpTrigger::Event(eureka_event(delta)),
        )
        .unwrap(),
    );
    let SkillTarget::Explicit(target_uid) = children[0].target else {
        panic!("lost-power child should have an explicit target");
    };
    target_uid
}

#[test]
fn lost_power_direct_cast_targets_the_eureka_consumer() {
    assert_eq!(
        crate::engine::skill::condition::registry::find_key(571017, "LostPower")
            .map(|definition| definition.child_target_source),
        Some(crate::engine::skill::condition::registry::ChildTargetSource::Event)
    );
    assert_eq!(
        child_target(
            condition(571017, "LostPower", &["1", "1"]),
            DefinitionKey::new(571017, "LostPower"),
            TargetContext::default(),
            -1,
        ),
        20
    );
}

#[test]
fn another_eureka_condition_ignores_stale_lost_power_context() {
    assert_eq!(
        child_target(
            condition(589, "PowerIncrChange", &["1", "1", "1"]),
            DefinitionKey::new(589, "PowerIncrChange"),
            TargetContext {
                runtime_target_uid: 20,
                lost_power_id: 1,
                lost_power_amount: 1,
                ..Default::default()
            },
            1,
        ),
        10
    );
}

#[test]
fn conduit_ultimate_compound_counts_only_device_ultimates() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3144),
                current_hp: Some(1),
                ex_skill: Some(31440131),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let emit = |skill_id| {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id: 116370013,
        }
        .into();
        invocation.condition_key = Some(DefinitionKey::new(792208, "UseDeviceSkill"));
        emit_all_ops(
            invocation,
            &managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &SkillOpTrigger::Event(BattleEvent::ConduitActivated(
                crate::engine::event::payload::ConduitActivatedEvent {
                    source_uid: 10,
                    team: 1,
                    skill_id,
                    power_id: 1,
                    activation_cost: 1,
                    spent: 1,
                },
            )),
        )
        .unwrap()
    };

    let ultimate = emit(31440131);
    assert!(matches!(
        ultimate.as_slice(),
        [RuleOp::Command(BattleCommand::Indicator(change))]
            if change.indicator_id == 3712 && change.amount == 1
    ));
    assert!(emit(31440111).is_empty());
}
