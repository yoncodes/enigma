use super::*;

#[test]
fn ezio_cloth_choice_runs_the_configured_skill_and_advances_qte_state() {
    crate::test_support::init_config();
    let entity = |uid, team_type, hp, attack| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(hp),
        ex_point_type: Some(if uid > 0 { 2 } else { 0 }),
        attr: Some(sonettobuf::HeroAttribute {
            hp: Some(hp),
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut runtime = BattleRuntime::new(Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1_000, 1_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, 100_000, 0)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: crate::engine::skill::rule::DefinitionKey::new(100000, "EzioProps"),
    };
    runtime
        .managers
        .execute_ex_point(
            crate::engine::manager::ex_point::ExPointCommand::ConfigureSynchronization(
                crate::engine::manager::ex_point::ExPointConfigureSynchronization {
                    origin,
                    target_uid: 10,
                    definition: crate::engine::manager::ex_point::SynchronizationDefinition::new(
                        [312301323, 312301333, 312301343],
                        4,
                        100,
                    )
                    .unwrap(),
                },
            ),
        )
        .unwrap();
    runtime
        .managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::Grant(
            crate::engine::manager::buff::BuffGrant {
                origin,
                source_uid: 10,
                target_uid: 10,
                buff_id: 229100,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))
        .unwrap();
    runtime.catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [312301323, 312301333, 312301343],
        [],
    );

    let request = UseClothSkillRequest {
        skill_id: Some(1),
        from_id: Some(10),
        to_id: Some(-1),
        r#type: Some(ClothSkillType::EzioBigSkill as i32),
    };
    let mut remaining = Vec::new();
    let mut replies = Vec::new();
    for _ in 0..4 {
        let reply = runtime.use_cloth_skill(request).unwrap();
        remaining.push(
            reply
                .round
                .as_ref()
                .unwrap()
                .fight_step
                .iter()
                .flat_map(|step| &step.act_effect)
                .find_map(|effect| {
                    (effect.effect_type
                        == Some(sonettobuf::effect_type_enum::EffectType::Buffupdate as i32))
                    .then_some(effect.buff.as_ref())
                    .flatten()
                    .filter(|buff| buff.buff_id == Some(229100))
                    .and_then(|buff| buff.act_common_params.as_deref())
                    .and_then(|params| params.split(['#', ',']).nth(1))
                    .and_then(|remaining| remaining.parse::<i32>().ok())
                })
                .unwrap(),
        );
        replies.push(reply);
    }

    let progress = runtime
        .managers
        .ex_point
        .synchronization_progress(10)
        .unwrap();
    assert_eq!((progress.completed_actions, progress.target_uid), (4, -1));
    assert!(progress.total_damage > 0);
    assert_eq!(remaining, [3, 2, 1, 0]);
    fn contains_skill(steps: &[sonettobuf::FightStep], skill_id: i32) -> bool {
        steps.iter().any(|step| {
            step.act_id == Some(skill_id)
                || step.act_effect.iter().any(|effect| {
                    effect.fight_step.as_ref().is_some_and(|nested| {
                        contains_skill(std::slice::from_ref(nested), skill_id)
                    })
                })
        })
    }
    assert!(contains_skill(
        &replies.last().unwrap().round.as_ref().unwrap().fight_step,
        312301343,
    ));
    fn buff_lifecycle(
        steps: &[sonettobuf::FightStep],
        buff_id: i32,
        lifecycle: &mut Vec<(i32, String)>,
    ) {
        for effect in steps.iter().flat_map(|step| &step.act_effect) {
            if let Some(buff) = effect
                .buff
                .as_ref()
                .filter(|buff| buff.buff_id == Some(buff_id))
            {
                lifecycle.push((
                    effect.effect_type.unwrap_or_default(),
                    buff.act_common_params.clone().unwrap_or_default(),
                ));
            }
            if let Some(nested) = effect.fight_step.as_ref() {
                buff_lifecycle(std::slice::from_ref(nested), buff_id, lifecycle);
            }
        }
    }
    let mut lifecycle = Vec::new();
    buff_lifecycle(
        &replies.last().unwrap().round.as_ref().unwrap().fight_step,
        229100,
        &mut lifecycle,
    );
    assert_eq!(
        lifecycle
            .iter()
            .map(|(effect, _)| *effect)
            .collect::<Vec<_>>(),
        [
            sonettobuf::effect_type_enum::EffectType::Buffupdate as i32,
            sonettobuf::effect_type_enum::EffectType::Buffupdate as i32,
            sonettobuf::effect_type_enum::EffectType::Buffdel as i32,
        ],
    );
    assert!(lifecycle[0].1.starts_with("10000#0,"));
    assert!(lifecycle[1].1.starts_with("10000#-1,"));
    assert_eq!(lifecycle[1].1, lifecycle[2].1);
    let terminal = lifecycle[1]
        .1
        .split(['#', ','])
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!((terminal[1], terminal[3]), (-1, -1));
    assert!(terminal[2] > progress.total_damage);
    assert!(!runtime.managers.buff.has_buff_id(10, 229100));
    assert!(runtime.use_cloth_skill(request).is_none());
}
