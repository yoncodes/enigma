use sonettobuf::effect_type_enum::EffectType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaloKind {
    Base,
    Master,
    Slave,
    LayerMaster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaloScope {
    AlliedTeam,
    OtherAllies,
    OpposingTeam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HaloCarrier {
    pub kind: HaloKind,
    pub scope: HaloScope,
    pub opcode: i32,
    pub type_name: &'static str,
    pub linked_buff_id: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HaloMarker {
    pub effect_type: EffectType,
}

pub fn carriers(catalog: crate::catalog::BattleCatalog, buff_id: i32) -> Vec<HaloCarrier> {
    catalog
        .buff_feature_tokens(buff_id)
        .into_iter()
        .filter_map(|token| {
            let mut parts = token.split('#');
            let opcode = parts.next()?.parse().ok()?;
            let definition = catalog.buff_act_definition(opcode)?;
            let scope = match parts.next()?.parse().ok()? {
                1 => HaloScope::AlliedTeam,
                2 => HaloScope::OtherAllies,
                3 => HaloScope::OpposingTeam,
                _ => return None,
            };
            let linked_buff_id = parts.next()?.parse().ok().filter(|id| *id > 0);
            match definition.kind {
                crate::engine::skill::buff_act::registry::BuffActKind::HaloBase => {
                    Some(HaloCarrier {
                        kind: HaloKind::Base,
                        scope,
                        opcode,
                        type_name: definition.key.type_name,
                        linked_buff_id,
                    })
                }
                crate::engine::skill::buff_act::registry::BuffActKind::MasterHalo => {
                    Some(HaloCarrier {
                        kind: HaloKind::Master,
                        scope,
                        opcode,
                        type_name: definition.key.type_name,
                        linked_buff_id,
                    })
                }
                crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo => {
                    Some(HaloCarrier {
                        kind: HaloKind::LayerMaster,
                        scope,
                        opcode,
                        type_name: definition.key.type_name,
                        linked_buff_id,
                    })
                }
                _ => None,
            }
        })
        .collect()
}

pub fn fanout_markers(catalog: crate::catalog::BattleCatalog, buff_id: i32) -> Vec<HaloMarker> {
    catalog
        .buff_feature_tokens(buff_id)
        .into_iter()
        .filter_map(|token| {
            let opcode = token.split('#').next()?.parse().ok()?;
            let effect_type = match catalog.buff_act_definition(opcode)?.kind {
                crate::engine::skill::buff_act::registry::BuffActKind::HaloBase => {
                    EffectType::Haloslave
                }
                crate::engine::skill::buff_act::registry::BuffActKind::MasterHalo
                | crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo => {
                    EffectType::Layerslavehalo
                }
                crate::engine::skill::buff_act::registry::BuffActKind::SlaveHalo => {
                    EffectType::Slavehalo
                }
                _ => return None,
            };
            Some(HaloMarker { effect_type })
        })
        .collect()
}

pub fn has_layer_master(catalog: crate::catalog::BattleCatalog, buff_id: i32) -> bool {
    catalog
        .buff_feature_tokens(buff_id)
        .into_iter()
        .any(|token| {
            token
                .split('#')
                .next()
                .and_then(|opcode| opcode.parse().ok())
                .and_then(|opcode| catalog.buff_act_definition(opcode))
                .is_some_and(|definition| {
                    definition.kind
                        == crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halo_base_uses_allied_team_scope_and_its_own_wire_marker() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            carriers(catalog, 109320111),
            vec![HaloCarrier {
                kind: HaloKind::Base,
                scope: HaloScope::AlliedTeam,
                opcode: 704,
                type_name: "HaloBase",
                linked_buff_id: None,
            }]
        );
        assert_eq!(
            fanout_markers(catalog, 109320111),
            vec![HaloMarker {
                effect_type: EffectType::Haloslave,
            }]
        );
    }
}
