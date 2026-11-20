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

pub fn effect_num(effect_type: i32, act_common_params: Option<&str>) -> i32 {
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
#[path = "marker/tests.rs"]
mod tests;
