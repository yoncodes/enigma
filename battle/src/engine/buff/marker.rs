use crate::engine::{manager::buff, skill::buff_act::wire::WirePhase};
use sonettobuf::effect_type_enum::EffectType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffMarker {
    pub effect_type: i32,
}

pub fn add_markers(buff_id: i32) -> Vec<BuffMarker> {
    markers_for(buff_id, WirePhase::Add)
}

pub fn static_markers(buff_id: i32) -> Vec<BuffMarker> {
    markers_for(buff_id, WirePhase::Static)
}

pub fn refresh_markers(buff_id: i32) -> Vec<BuffMarker> {
    markers_for(buff_id, WirePhase::Refresh)
}

pub fn effect_num(effect_type: i32, buff_id: i32, act_common_params: Option<&str>) -> i32 {
    if effect_type == EffectType::Exskillpointchange as i32 {
        return crate::engine::manager::buff::BuffManager::configured_features(buff_id)
            .iter()
            .filter(|feature| {
                crate::engine::skill::buff_act::is_kind(
                    feature,
                    crate::engine::skill::buff_act::registry::BuffActKind::ExSkillPointChange,
                )
            })
            .filter_map(|feature| feature.values.get(1))
            .copied()
            .sum();
    }
    if ![
        EffectType::Fixattrteamenergy as i32,
        EffectType::Fixattrteamenergyandbuff as i32,
    ]
    .contains(&effect_type)
    {
        return 0;
    }
    act_common_params
        .and_then(|raw| raw.split('#').nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

fn markers_for(buff_id: i32, phase: WirePhase) -> Vec<BuffMarker> {
    buff::wire_markers(buff_id, phase)
        .into_iter()
        .map(|effect_type| BuffMarker { effect_type })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::effect_type_enum::EffectType;

    #[test]
    fn team_energy_attribute_markers_use_the_committed_snapshot() {
        assert_eq!(
            effect_num(EffectType::Fixattrteamenergy as i32, 0, Some("882#15")),
            15
        );
        assert_eq!(
            effect_num(
                EffectType::Fixattrteamenergyandbuff as i32,
                0,
                Some("883#300"),
            ),
            300
        );
    }

    #[test]
    fn ultimate_cost_marker_uses_the_registered_buff_feature_value() {
        crate::test_support::init_config();

        assert_eq!(
            effect_num(EffectType::Exskillpointchange as i32, 2220012, None),
            -4
        );
    }

    #[test]
    fn count_buff_emits_configured_feature_markers() {
        crate::test_support::init_config();
        assert_eq!(
            add_markers(31020111)
                .into_iter()
                .map(|marker| marker.effect_type)
                .collect::<Vec<_>>(),
            vec![EffectType::Addtotarget as i32, EffectType::None as i32]
        );
        assert_eq!(
            add_markers(31020114)[0].effect_type,
            EffectType::None as i32
        );
        assert_eq!(
            add_markers(30620111)[0].effect_type,
            EffectType::None as i32
        );
        assert_eq!(
            add_markers(31200124)
                .into_iter()
                .map(|marker| marker.effect_type)
                .collect::<Vec<_>>(),
            vec![EffectType::None as i32, EffectType::Cureupbylosthp as i32]
        );
        assert_eq!(
            add_markers(308801211)
                .into_iter()
                .map(|marker| marker.effect_type)
                .collect::<Vec<_>>(),
            vec![EffectType::None as i32, EffectType::Monsterlabelbuff as i32]
        );
    }

    #[test]
    fn ex_point_max_buff_uses_its_committed_resource_change_without_a_duplicate_marker() {
        crate::test_support::init_config();

        assert!(add_markers(31140141).is_empty());
        assert!(refresh_markers(31140141).is_empty());
    }

    #[test]
    fn wound_feature_emits_its_dot_marker() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(530000412)[0].effect_type,
            EffectType::Dot as i32
        );
    }

    #[test]
    fn buff_add_limit_emits_its_none_marker() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(31130113)[0].effect_type,
            EffectType::None as i32
        );
    }

    #[test]
    fn unregistered_recorded_buff_layer_counter_has_no_wire_marker() {
        crate::test_support::init_config();

        assert!(add_markers(30650203).is_empty());
        assert!(static_markers(30650203).is_empty());
        assert!(refresh_markers(30650203).is_empty());
    }

    #[test]
    fn unregistered_attack_reaction_feature_does_not_add_a_wire_marker() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(30870301),
            vec![BuffMarker {
                effect_type: EffectType::Slavehalo as i32,
            }]
        );
        assert!(refresh_markers(30870301).is_empty());
    }

    #[test]
    fn from_the_depths_buffs_use_their_configured_markers() {
        crate::test_support::init_config();

        assert_eq!(add_markers(433021)[0].effect_type, EffectType::Cure as i32);
        assert_eq!(add_markers(433031)[0].effect_type, EffectType::None as i32);
    }

    #[test]
    fn beryl_halo_uses_the_radiance_marker() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(31340001),
            vec![BuffMarker {
                effect_type: EffectType::Radiance as i32,
            }]
        );
        assert!(refresh_markers(31340001).is_empty());
    }

    #[test]
    fn dream_visit_uses_its_configured_dizzy_marker() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(23390081),
            vec![BuffMarker {
                effect_type: EffectType::Dizzy as i32,
            }]
        );
        assert!(refresh_markers(23390081).is_empty());
    }

    #[test]
    fn assist_boss_afflatus_buff_marks_only_its_add() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(116331900),
            vec![
                BuffMarker {
                    effect_type: EffectType::Attr as i32,
                },
                BuffMarker {
                    effect_type: EffectType::Attr as i32,
                },
                BuffMarker {
                    effect_type: EffectType::Attr as i32,
                },
                BuffMarker {
                    effect_type: EffectType::Attr as i32,
                },
                BuffMarker {
                    effect_type: EffectType::Attr as i32,
                },
                BuffMarker {
                    effect_type: EffectType::None as i32,
                },
            ]
        );
        assert_eq!(refresh_markers(116331900).len(), 5);
    }

    #[test]
    fn unregistered_assist_boss_cooldown_has_no_wire_marker() {
        crate::test_support::init_config();

        assert!(add_markers(4700209).is_empty());
        assert!(static_markers(4700209).is_empty());
        assert!(refresh_markers(4700209).is_empty());
    }

    #[test]
    fn impromptu_round_buffs_emit_their_configured_markers() {
        crate::test_support::init_config();

        assert_eq!(
            (
                add_markers(30480211),
                add_markers(30480212),
                add_markers(30480231),
                add_markers(30483),
                add_markers(31130123),
            ),
            (
                vec![BuffMarker {
                    effect_type: EffectType::Addsplitemitternum as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::Emitternumchange as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::None as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::Buffreplace as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::Addtotarget as i32,
                }],
            )
        );
    }

    #[test]
    fn refresh_markers_do_not_promote_buff_acts_to_packet_markers() {
        crate::test_support::init_config();

        assert_eq!(
            refresh_markers(31130122),
            vec![BuffMarker {
                effect_type: EffectType::None as i32,
            }]
        );
        assert!(refresh_markers(31130123).is_empty());
    }

    #[test]
    fn flutterpage_channel_buffs_emit_their_configured_markers() {
        crate::test_support::init_config();

        assert_eq!(
            (
                add_markers(31050131),
                add_markers(31050132),
                add_markers(31050145),
            ),
            (
                vec![BuffMarker {
                    effect_type: EffectType::None as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::Expointcantadd as i32,
                }],
                vec![
                    BuffMarker {
                        effect_type: EffectType::None as i32,
                    },
                    BuffMarker {
                        effect_type: EffectType::None as i32,
                    },
                    BuffMarker {
                        effect_type: EffectType::None as i32,
                    },
                ],
            )
        );
    }

    #[test]
    fn share_hurt_and_provoke_emit_their_configured_add_markers() {
        crate::test_support::init_config();

        assert_eq!(
            (
                add_markers(31090121),
                add_markers(229103),
                add_markers(2292031),
            ),
            (
                vec![BuffMarker {
                    effect_type: EffectType::None as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::None as i32,
                }],
                vec![BuffMarker {
                    effect_type: EffectType::None as i32,
                }],
            )
        );
    }

    #[test]
    fn lost_hp_blood_pool_bonus_marks_only_its_initial_add() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(31260121),
            vec![BuffMarker {
                effect_type: EffectType::None as i32,
            }]
        );
        assert!(static_markers(31260121).is_empty());
        assert!(refresh_markers(31260121).is_empty());
    }

    #[test]
    fn conditional_critical_buff_marks_only_its_initial_add() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(31280112),
            vec![
                BuffMarker {
                    effect_type: EffectType::Attr as i32,
                },
                BuffMarker {
                    effect_type: EffectType::None as i32,
                },
            ]
        );
        assert!(refresh_markers(31280112).is_empty());
    }

    #[test]
    fn joe_shield_counter_marks_only_its_initial_add() {
        crate::test_support::init_config();

        assert_eq!(
            add_markers(30940121),
            vec![BuffMarker {
                effect_type: EffectType::Shield as i32,
            }]
        );
        assert_eq!(
            add_markers(30940181),
            vec![BuffMarker {
                effect_type: EffectType::None as i32,
            }]
        );
        assert!(static_markers(30940121).is_empty());
        assert!(refresh_markers(30940121).is_empty());
        assert!(static_markers(30940181).is_empty());
        assert!(refresh_markers(30940181).is_empty());
    }
}
