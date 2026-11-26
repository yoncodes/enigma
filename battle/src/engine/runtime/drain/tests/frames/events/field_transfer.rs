use super::*;

#[test]
fn field_transfer_reaction_uses_the_published_owner() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![BuffInfo {
                    uid: Some(110),
                    buff_id: Some(31350211),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let event = BattleEvent::FieldTransferred {
        team: 1,
        amount: 25,
    };
    let cast = [0, 1, 0];
    let settlement = [0, 1];
    for owner in [None, Some(settlement.as_slice())] {
        let reactions = dispatch_event_groups(
            ReactionAccess {
                pool: &pool,
                managers: &managers,
                catalog: &SkillEffectCatalog::default(),
                determinism: &mut RoundDeterminism::default(),
            },
            &mut DrainState::default(),
            &[vec![event.clone()]],
            EventFrame {
                parent_path: &cast,
                reuse_path: &cast,
                action_path: None,
                entry_halo_scope: Some(&cast),
                current_skill: None,
            },
            ReactionSelection {
                include_attack_consumption: false,
                execute_unscoped_after_action: true,
                publication_phase:
                    crate::engine::event::subscription::PublicationPhase::AfterPublish,
                owner_uids: None,
                lane: None,
            },
            None,
            owner,
        )
        .unwrap();
        let destinations = reactions
            .into_ordered()
            .into_iter()
            .filter_map(|reaction| {
                matches!(
                    reaction.frame_owner,
                    Some(FrameOwner::BuffAct { key, .. })
                        if key == DefinitionKey::new(1076, "PerElectricAddBuff")
                )
                .then_some(reaction.frame_path)
            })
            .collect::<Vec<_>>();
        let expected = owner.unwrap_or(&cast).to_vec();
        assert_eq!(destinations, [Some(expected.clone()), Some(expected)]);
    }
}
