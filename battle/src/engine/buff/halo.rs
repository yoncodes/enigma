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

pub fn carriers(buff_id: i32) -> Vec<HaloCarrier> {
    feature_tokens(buff_id)
        .filter_map(|token| {
            let mut parts = token.split('#');
            let opcode = parts.next()?.parse().ok()?;
            let definition = registry_definition(opcode)?;
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

pub fn fanout_markers(buff_id: i32) -> Vec<HaloMarker> {
    feature_tokens(buff_id)
        .filter_map(|token| {
            let opcode = token.split('#').next()?.parse().ok()?;
            let effect_type = match registry_definition(opcode)?.kind {
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

pub fn has_layer_master(buff_id: i32) -> bool {
    feature_tokens(buff_id).any(|token| {
        token
            .split('#')
            .next()
            .and_then(|opcode| opcode.parse().ok())
            .and_then(registry_definition)
            .is_some_and(|definition| {
                definition.kind
                    == crate::engine::skill::buff_act::registry::BuffActKind::LayerMasterHalo
            })
    })
}

fn registry_definition(
    opcode: i32,
) -> Option<&'static crate::engine::skill::buff_act::registry::BuffActDefinition> {
    let act = config::try_get()?.buff_act.get(opcode)?;
    crate::engine::skill::buff_act::registry::find(opcode, &act.r#type)
}

fn feature_tokens(buff_id: i32) -> impl Iterator<Item = String> {
    config::configs::get()
        .skill_buff
        .get(buff_id)
        .map(|row| row.features.clone())
        .unwrap_or_default()
        .split('|')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halo_base_uses_allied_team_scope_and_its_own_wire_marker() {
        crate::test_support::init_config();

        assert_eq!(
            carriers(109320111),
            vec![HaloCarrier {
                kind: HaloKind::Base,
                scope: HaloScope::AlliedTeam,
                opcode: 704,
                type_name: "HaloBase",
                linked_buff_id: None,
            }]
        );
        assert!(fanout_markers(109320111).is_empty());
    }
}
