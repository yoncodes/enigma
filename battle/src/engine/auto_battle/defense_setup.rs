use crate::engine::{
    entity::attr::AttrId,
    manager::BattleManagers,
    skill::{
        buff_act::registry::{self, BuffActKind},
        target::request::ALL_ENEMIES_TARGET_CODE,
    },
};

pub(super) fn available(
    target_code: i32,
    targets: &[i64],
    buffs: &[i32],
    managers: &BattleManagers,
) -> bool {
    target_code == ALL_ENEMIES_TARGET_CODE
        && buffs.iter().any(|&buff_id| {
            let buff_type = managers.catalog().buff_type_id(buff_id);
            let key = if buff_type > 0 { buff_type } else { buff_id };
            lowers_defense(buff_id, managers)
                && targets
                    .iter()
                    .any(|&uid| !managers.buff.has_active_buff_id_or_type(uid, key))
        })
}

fn lowers_defense(buff_id: i32, managers: &BattleManagers) -> bool {
    managers
        .buff
        .definition_features(buff_id)
        .iter()
        .any(|feature| {
            let [opcode, attribute, delta] = feature.values.as_slice() else {
                return false;
            };
            *delta < 0
                && matches!(
                    AttrId::from_raw(*attribute),
                    Some(AttrId::RealityDef | AttrId::MentalDef | AttrId::CriticalDef)
                )
                && registry::find(*opcode, &feature.act_type)
                    .is_some_and(|definition| definition.kind == BuffActKind::Attr)
        })
}
