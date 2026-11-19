use super::*;

#[test]
fn self_replacement_publishes_removal_and_addition_with_their_own_reactions() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1000),
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
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::default();
    let grant = || {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: 10,
            target_uid: 10,
            buff_id: 31460121,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    };
    let drain = |managers: &mut BattleManagers| {
        let mut frames = Vec::new();
        let root = push_root(&mut frames, FrameOwner::Command, FrameTrigger::Active);
        let mut queue = VecDeque::from([QueuedOp {
            op: grant(),
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
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            &mut queue,
            frames,
        )
        .unwrap()
    };
    drain(&mut managers);

    let result = drain(&mut managers);
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
