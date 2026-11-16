use super::*;

#[test]
fn settlement_owner_and_feedback_target_remain_distinct() {
    crate::test_support::init_config();
    let passive = 31402161;
    let feedback = 31402131;
    let inspection = 31400006;
    let entity = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(100000),
        attr: Some(HeroAttribute {
            hp: Some(100000),
            attack: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                model_id: Some(3140),
                passive_skill: vec![passive],
                buffs: vec![
                    BuffInfo {
                        uid: Some(100),
                        buff_id: Some(inspection),
                        from_uid: Some(10),
                        duration: Some(2),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(101),
                        buff_id: Some(31400003),
                        from_uid: Some(10),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1147),
                            param: vec![400, 5000],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    },
                ],
                ..entity(10, 1)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let result = run_grouped_owner_event(
        &mut managers,
        &TargetPool::from_fight(&fight),
        &SkillEffectCatalog::from_fight(config::configs::get(), &fight),
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundEndAfterSideFinalExpiry),
        &[10],
        ReactionLane::Skills,
    )
    .unwrap();

    fn find(step: &sonettobuf::FightStep, id: i32) -> Option<&sonettobuf::FightStep> {
        (step.act_id == Some(id)).then_some(step).or_else(|| {
            step.act_effect
                .iter()
                .filter_map(|effect| effect.fight_step.as_ref())
                .find_map(|child| find(child, id))
        })
    }
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let observer = steps.iter().find_map(|step| find(step, passive)).unwrap();
    assert_eq!(observer.to_id, Some(10));
    assert_eq!(find(observer, feedback).unwrap().to_id, Some(-1));
    assert!(managers.hp.current(-1) < 100000);
}
