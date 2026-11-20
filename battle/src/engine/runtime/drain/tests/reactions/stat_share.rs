use super::*;
use crate::engine::skill::buff_act::raspberry;

#[test]
fn each_change_attr_one_way_uses_max_hp_without_capacity() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [(10, 11_844), (11, 24_733), (12, 20_011)]
                .into_iter()
                .map(|(uid, hp)| FightEntityInfo {
                    uid: Some(uid),
                    team_type: Some(1),
                    current_hp: Some(hp),
                    attr: Some(HeroAttribute {
                        hp: Some(hp),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let add_origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let grant = |source_uid, target_uid, buff_id| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin: add_origin,
            source_uid,
            target_uid,
            buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let drain = |managers: &mut BattleManagers, ops: Vec<RuleOp>| {
        run(
            managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            ops,
        )
        .unwrap()
    };

    drain(
        &mut managers,
        vec![
            grant(10, 10, 436815),
            grant(12, 10, 308801922),
            grant(12, 10, 31250261),
        ],
    );
    let buff_uid = managers.buff.buff_id_uid(10, 31250261).unwrap();
    drain(
        &mut managers,
        vec![RuleOp::Command(BattleCommand::RaspberryCapacity(
            raspberry::CapacityCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1042, "Raspberry"),
                },
                source_uid: 12,
                target_uid: 10,
                buff_uid,
                buff_act_id: 1042,
                current: 1_845,
                cap: 3_322,
                delta: 1_845,
            },
        ))],
    );
    let source = managers.hp.get(10);
    drain(&mut managers, vec![grant(10, 11, 31460134)]);

    assert_eq!(managers.hp.get(10), source);
    assert_eq!(managers.hp.max(10), 16_874);
    let share = managers.buff.buff_id_uid(11, 31460134).unwrap();
    assert_eq!(
        managers.buff.fixed_attribute_value(share, AttrId::Hp),
        Some(1_042)
    );
    assert_eq!(managers.hp.max(11), 25_775);
    assert_eq!(managers.hp.current(11), 25_775);

    drain(&mut managers, vec![grant(12, 12, 31460121)]);
    let loan = managers.buff.buff_id_uid(10, 308801922).unwrap();
    let remove = |target_uid, buff_uid| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin: add_origin,
            target_uid,
            selector: BuffRemoveSelector::Uid(buff_uid),
        })))
    };
    drain(&mut managers, vec![remove(10, loan), remove(10, buff_uid)]);
    drain(&mut managers, vec![grant(10, 12, 31460134)]);
    let share = managers.buff.buff_id_uid(12, 31460134).unwrap();
    assert_eq!(
        managers.buff.fixed_attribute_value(share, AttrId::Hp),
        Some(1_042)
    );
}

#[test]
fn each_change_attr_one_way_reads_cloak_from_registered_buff() {
    crate::test_support::init_config();
    let mut source = FightEntityInfo {
        uid: Some(10),
        team_type: Some(1),
        current_hp: Some(13_428),
        attr: Some(HeroAttribute {
            hp: Some(13_428),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(77),
            buff_id: Some(31250261),
            from_uid: Some(12),
            act_info: vec![BuffActInfo {
                act_id: Some(1042),
                param: vec![400, 3_322],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![source.clone()],
            ..Default::default()
        }),
        ..Default::default()
    });
    assert_eq!(managers.hp.max_without_capacity(10), 13_028);
    let changes = raspberry::execute_capacity(
        &mut managers,
        raspberry::CapacityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1042, "Raspberry"),
            },
            source_uid: 12,
            target_uid: 10,
            buff_uid: 77,
            buff_act_id: 1042,
            current: 0,
            cap: 3_322,
            delta: -400,
        },
    )
    .unwrap();
    assert_eq!(changes.hp.max_hp.unwrap().after_max, 13_028);
    assert_eq!(managers.hp.max_without_capacity(10), 13_028);

    source.current_hp = Some(13_028);
    source.attr.as_mut().unwrap().hp = Some(13_028);
    managers.register_entity(&source);
    assert_eq!(managers.hp.max_without_capacity(10), 13_028);
    let changes = raspberry::execute_capacity(
        &mut managers,
        raspberry::CapacityCommand {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(1042, "Raspberry"),
            },
            source_uid: 12,
            target_uid: 10,
            buff_uid: 77,
            buff_act_id: 1042,
            current: 0,
            cap: 3_322,
            delta: -400,
        },
    )
    .unwrap();
    assert_eq!(changes.hp.max_hp.unwrap().after_max, 12_628);
    assert_eq!(managers.hp.max_without_capacity(10), 13_028);
}
