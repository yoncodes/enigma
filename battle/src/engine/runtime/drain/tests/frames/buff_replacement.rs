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
    drain_grant_from(pool, managers, 10, buff_id)
}

fn drain_grant_from(
    pool: &TargetPool,
    managers: &mut BattleManagers,
    source_uid: i64,
    buff_id: i32,
) -> DrainResult {
    drain_grant_to(pool, managers, source_uid, 10, buff_id)
}

fn drain_grant_to(
    pool: &TargetPool,
    managers: &mut BattleManagers,
    source_uid: i64,
    target_uid: i64,
    buff_id: i32,
) -> DrainResult {
    let mut frames = Vec::new();
    let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
    let mut queue = VecDeque::from([QueuedOp {
        op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid,
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

#[test]
fn one_way_snapshot_markers_lead_the_add_and_the_replacement() {
    use sonettobuf::effect_type_enum::EffectType;
    crate::test_support::init_config();
    let hero = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1000),
        attr: Some(HeroAttribute {
            hp: Some(1000),
            attack: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![hero(10), hero(20)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let kinds = |result: &DrainResult| {
        wire(result)
            .into_iter()
            .filter_map(|(effect_type, _, buff_act_id)| {
                [
                    (EffectType::Buffactinfoupdate as i32, "info"),
                    (EffectType::Maxhpchange as i32, "max"),
                    (EffectType::Currenthpchange as i32, "current"),
                    (EffectType::Buffdel as i32, "del"),
                    (EffectType::Buffadd as i32, "add"),
                ]
                .into_iter()
                .find(|(kind, _)| *kind == effect_type)
                .map(|(_, name)| {
                    if buff_act_id == 0 {
                        name.to_owned()
                    } else {
                        format!("{name}({buff_act_id})")
                    }
                })
            })
            .collect::<Vec<_>>()
    };

    let fresh = drain_grant_from(&pool, &mut managers, 20, 31460131);
    assert_eq!(kinds(&fresh), ["info", "info", "max", "current", "add"]);

    let replaced = drain_grant_from(&pool, &mut managers, 20, 31460131);
    assert_eq!(
        kinds(&replaced),
        [
            "info",
            "info",
            "max(1131)",
            "max(1131)",
            "del",
            "max",
            "add"
        ]
    );
}

#[test]
fn layered_halo_snapshot_replacement_replaces_each_copy_in_its_own_step() {
    use sonettobuf::effect_type_enum::EffectType;
    crate::test_support::init_config();
    let hero = |uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1000),
        attr: Some(HeroAttribute {
            hp: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![hero(-4), hero(-1), hero(-2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    fn collect(steps: &[sonettobuf::FightStep], out: &mut Vec<sonettobuf::ActEffect>) {
        for step in steps {
            for effect in &step.act_effect {
                out.push(effect.clone());
                if let Some(child) = &effect.fight_step {
                    collect(std::slice::from_ref(child), out);
                }
            }
        }
    }
    let per_copy = |result: &DrainResult| {
        let mut effects = Vec::new();
        collect(
            &crate::engine::packet::timeline::project(&result.frames).unwrap(),
            &mut effects,
        );
        let owners = effects
            .iter()
            .filter(|effect| effect.effect_type == Some(EffectType::Buffadd as i32))
            .filter_map(|effect| Some((effect.buff.as_ref()?.uid?, effect.target_id?)))
            .collect::<std::collections::HashMap<_, _>>();
        let mut copies = std::collections::BTreeMap::<i64, Vec<String>>::new();
        for effect in effects {
            let effect_type = effect.effect_type.unwrap_or_default();
            let act = effect.buff_act_id.unwrap_or_default();
            let (name, target) = if effect_type == EffectType::Buffactinfoupdate as i32
                && effect.target_id == Some(0)
            {
                (
                    "info",
                    owners.get(&effect.reserve_id.unwrap_or_default()).copied(),
                )
            } else if effect_type == EffectType::Maxhpchange as i32 {
                ("max", effect.target_id)
            } else if effect_type == EffectType::Buffdel as i32 {
                ("del", effect.target_id)
            } else if effect_type == EffectType::Buffadd as i32 {
                ("add", effect.target_id)
            } else {
                continue;
            };
            copies
                .entry(target.unwrap_or_default())
                .or_default()
                .push(if act == 0 {
                    name.to_owned()
                } else {
                    format!("{name}({act})")
                });
        }
        let mut copies = copies.into_iter().collect::<Vec<_>>();
        copies.sort_by_key(|(target, _)| *target);
        copies
    };

    let fresh = drain_grant_to(&pool, &mut managers, -4, -4, 31560152);
    let replaced = drain_grant_to(&pool, &mut managers, -4, -4, 31560152);
    let expected = |lines: &[&str]| {
        [-4, -2, -1]
            .into_iter()
            .map(|uid| (uid, lines.iter().map(|line| (*line).to_owned()).collect()))
            .collect::<Vec<_>>()
    };
    assert_eq!(per_copy(&fresh), expected(&["info", "add"]));
    assert_eq!(
        per_copy(&replaced),
        expected(&["info", "max(1162)", "del", "add"])
    );
    for uid in [-4, -1, -2] {
        assert_eq!(managers.buff.buff_id_amount(uid, 31560152), 1);
    }
}
