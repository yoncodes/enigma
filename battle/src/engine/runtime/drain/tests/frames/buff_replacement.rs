use super::*;

fn max_hp_fixture() -> (TargetPool, BattleManagers) {
    max_hp_fixture_at(1000)
}

fn max_hp_fixture_at(current_hp: i32) -> (TargetPool, BattleManagers) {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(current_hp),
                attr: Some(HeroAttribute {
                    hp: Some(1000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    (
        TargetPool::from_fight(&fight),
        BattleManagers::seeded(&fight),
    )
}

fn drain_grant(pool: &TargetPool, managers: &mut BattleManagers, buff_id: i32) -> DrainResult {
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))),
        trigger: SkillOpTrigger::Active,
        skill_execution: None,
        frame_path: Some(root),
        parent_path: None,
        frame_group: None,
        independent_parent_group: None,
        frame_owner: None,
        subscriber: None,
        caster_frame: None,
    }]);
    drain_queue_with_frames(
        managers,
        pool,
        &SkillEffectCatalog::default(),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &mut queue,
        frames,
    )
    .unwrap()
}

fn wire(result: &DrainResult) -> Vec<(i32, i32, i32)> {
    fn collect(steps: &[sonettobuf::FightStep], out: &mut Vec<(i32, i32, i32)>) {
        for step in steps {
            for effect in &step.act_effect {
                out.push((
                    effect.effect_type.unwrap_or_default(),
                    effect.effect_num.unwrap_or_default(),
                    effect.buff_act_id.unwrap_or_default(),
                ));
                if let Some(child) = &effect.fight_step {
                    collect(std::slice::from_ref(child), out);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(
        &crate::engine::packet::timeline::project(&result.frames).unwrap(),
        &mut out,
    );
    out
}

#[test]
fn self_replacement_publishes_removal_and_addition_with_their_own_reactions() {
    let (pool, mut managers) = max_hp_fixture();
    drain_grant(&pool, &mut managers, 31460121);

    let result = drain_grant(&pool, &mut managers, 31460121);
    let order = result.frames[0]
        .items
        .iter()
        .filter_map(|item| match item {
            FrameItem::Change(change) => match change.as_ref() {
                crate::engine::runtime::change::BattleChange::Buff(changes) => {
                    Some(format!("{:?}", changes.publish_part))
                }
                crate::engine::runtime::change::BattleChange::Hp(changes) => changes
                    .max_hp
                    .map(|max_hp| format!("max hp {}", max_hp.after_max)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        ["Lead", "max hp 1000", "Removal", "max hp 1120", "Addition"]
    );
}

#[test]
fn attr_self_replacement_clamps_silently_and_keeps_current_hp_on_the_add() {
    use sonettobuf::effect_type_enum::EffectType;
    let (pool, mut managers) = max_hp_fixture();
    drain_grant(&pool, &mut managers, 31460121);
    assert_eq!(managers.hp.current(10), 1120);

    let result = drain_grant(&pool, &mut managers, 31460121);
    let hp = wire(&result)
        .into_iter()
        .filter(|(effect_type, _, _)| {
            [
                EffectType::Maxhpchange as i32,
                EffectType::Currenthpchange as i32,
                EffectType::Buffdel as i32,
                EffectType::Buffadd as i32,
            ]
            .contains(effect_type)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hp,
        [
            (EffectType::Maxhpchange as i32, 1000, 100),
            (EffectType::Buffdel as i32, 0, 0),
            (EffectType::Maxhpchange as i32, 1120, 0),
            (EffectType::Maxhpchange as i32, 1120, 0),
            (EffectType::Buffadd as i32, 31460121, 0),
        ]
    );
    assert_eq!(managers.hp.max(10), 1120);
    assert_eq!(managers.hp.current(10), 1000);
}

#[test]
fn cross_rank_self_replacement_tags_the_removal_and_raises_current_hp_on_the_add() {
    use sonettobuf::effect_type_enum::EffectType;
    let (pool, mut managers) = max_hp_fixture_at(500);
    drain_grant(&pool, &mut managers, 31460121);
    assert_eq!(managers.hp.current(10), 620);

    let result = drain_grant(&pool, &mut managers, 31460122);
    let hp = wire(&result)
        .into_iter()
        .filter(|(effect_type, _, _)| {
            [
                EffectType::Maxhpchange as i32,
                EffectType::Currenthpchange as i32,
                EffectType::Buffdel as i32,
                EffectType::Buffadd as i32,
            ]
            .contains(effect_type)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hp,
        [
            (EffectType::Maxhpchange as i32, 1000, 100),
            (EffectType::Buffdel as i32, 0, 0),
            (EffectType::Maxhpchange as i32, 1130, 0),
            (EffectType::Currenthpchange as i32, 750, 0),
            (EffectType::Maxhpchange as i32, 1130, 0),
            (EffectType::Currenthpchange as i32, 750, 0),
            (EffectType::Buffadd as i32, 31460122, 0),
        ]
    );
    assert_eq!(managers.hp.current(10), 750);
}

#[test]
fn rank_down_self_replacement_keeps_current_hp_on_the_add() {
    use sonettobuf::effect_type_enum::EffectType;
    let (pool, mut managers) = max_hp_fixture_at(500);
    drain_grant(&pool, &mut managers, 31460122);
    assert_eq!(managers.hp.current(10), 630);

    let result = drain_grant(&pool, &mut managers, 31460121);
    let hp = wire(&result)
        .into_iter()
        .filter(|(effect_type, _, _)| {
            [
                EffectType::Maxhpchange as i32,
                EffectType::Currenthpchange as i32,
                EffectType::Buffdel as i32,
                EffectType::Buffadd as i32,
            ]
            .contains(effect_type)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hp,
        [
            (EffectType::Maxhpchange as i32, 1000, 100),
            (EffectType::Buffdel as i32, 0, 0),
            (EffectType::Maxhpchange as i32, 1120, 0),
            (EffectType::Maxhpchange as i32, 1120, 0),
            (EffectType::Buffadd as i32, 31460121, 0),
        ]
    );
    assert_eq!(managers.hp.current(10), 630);
}
