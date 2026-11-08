use super::*;
use crate::engine::manager::card::{CardInsertSovereign, SovereignEnhancement};

#[test]
fn played_enhancements_apply_before_damage_at_the_selected_main_target() {
    init_config();
    for (levels, critical, amounts) in [
        ([0, 0, 0, 0], false, vec![4400]),
        ([1, 0, 0, 0], false, vec![9900]),
        ([0, 1, 0, 0], false, vec![7700, 7700]),
        ([0, 0, 1, 0], false, vec![4928]),
        ([0, 0, 0, 1], true, vec![7128]),
        ([1, 1, 1, 1], false, vec![14784, 9856]),
    ] {
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3147),
                    team_type: Some(1),
                    current_hp: Some(1000),
                    attr: Some(HeroAttribute {
                        hp: Some(1000),
                        attack: Some(1000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: [-1, -2, -3]
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        team_type: Some(2),
                        current_hp: Some(1_000_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })
                    .to_vec(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            10,
            &sonettobuf::HeroExAttribute {
                cri_dmg: Some(1500),
                ..Default::default()
            },
        );
        managers
            .execute_card(CardCommand::InsertSovereign(CardInsertSovereign {
                origin: CARD_PLAY_ORIGIN,
                target_uid: 10,
                skill_id: 31472131,
                hero_id: 3147,
                max_position: 9,
                team_type: 1,
            }))
            .unwrap();
        for (kind, count) in [
            SovereignEnhancement::Single,
            SovereignEnhancement::Mass,
            SovereignEnhancement::Support,
            SovereignEnhancement::Recovery,
        ]
        .into_iter()
        .zip(levels)
        {
            for _ in 0..count {
                managers
                    .execute_card(CardCommand::ChargeSovereign {
                        origin: CARD_PLAY_ORIGIN,
                        owner_uid: 10,
                        kind,
                        team_type: 1,
                    })
                    .unwrap();
            }
        }
        managers
            .execute_card(CardCommand::UnlockSovereign {
                origin: CARD_PLAY_ORIGIN,
                owner_uid: 10,
                team_type: 1,
            })
            .unwrap();
        managers
            .execute_card(CardCommand::Play(CardPlay {
                origin: CARD_PLAY_ORIGIN,
                hand_index: 0,
                target_uid: Some(-2),
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [31472131], []);
        let mut skill = crate::engine::skill::action::SkillInvocation::from(
            crate::engine::skill::action::SkillRequest {
                source_uid: 10,
                skill_id: 31472131,
            },
        );
        skill.card_index = 1;
        skill.target = crate::engine::skill::action::SkillTarget::Explicit(-2);
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(31472131, 10, [critical; 3]);
        let result = run_active_action(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            ActiveActionRequest {
                skill,
                grants_ex_point: false,
                grant_after_action: false,
                composition_rewards: Vec::new(),
                prelude: Vec::new(),
            },
        )
        .unwrap();
        let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
        let step = steps
            .iter()
            .find(|step| step.act_id == Some(31472131))
            .unwrap();
        let markers = step
            .act_effect
            .iter()
            .filter(|effect| effect.effect_type == Some(380))
            .map(|effect| (effect.target_id, effect.effect_num, effect.effect_num1))
            .collect::<Vec<_>>();
        let expected = levels
            .into_iter()
            .enumerate()
            .filter(|(_, level)| *level > 0)
            .map(|(index, level)| (Some(-2), Some(index as i32 + 1), Some(level)))
            .collect::<Vec<_>>();
        assert_eq!(markers, expected, "levels={levels:?}");
        let hits = step
            .act_effect
            .iter()
            .filter(|effect| effect.hurt_info.is_some())
            .map(|effect| (effect.target_id.unwrap(), effect.effect_num.unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(
            hits.iter().map(|(_, amount)| *amount).collect::<Vec<_>>(),
            amounts,
            "levels={levels:?}, critical={critical}"
        );
        assert_eq!(hits[0].0, -2);
        if hits.len() > 1 {
            assert_eq!(hits[1].0, -1);
        }
        assert_eq!(
            managers.card.played()[0].card.card_dataes[0].value,
            Some(format!(
                "{{\"lock\":false,\"strengthen\":{{\"1\":{},\"2\":{},\"3\":{},\"4\":{}}}}}",
                levels[0], levels[1], levels[2], levels[3]
            ))
        );
        assert_eq!(managers.hp.current(10), 1000);
        assert_eq!(
            managers
                .attribute
                .get(10, crate::engine::entity::attr::AttrId::CriticalDmg),
            1500
        );
    }
}
