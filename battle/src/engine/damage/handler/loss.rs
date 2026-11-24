use crate::engine::{
    damage::scale_permille, entity::attr::AttrId, event::payload::BattleEvent,
    manager::BattleManagers, skill::effect::ParsedBehavior,
};

pub(super) fn amount(
    target_uid: i64,
    managers: &BattleManagers,
    behavior: &ParsedBehavior,
    event: Option<&BattleEvent>,
) -> Option<i32> {
    let [mode, raw_attr, permille] = behavior.args.as_slice() else {
        return None;
    };
    let attr = AttrId::from_raw(*raw_attr)?;
    let hp = managers.hp.get(target_uid);
    let basis = if *mode == 1 {
        hp.current
    } else {
        match attr {
            AttrId::CurrentHp => hp.current,
            AttrId::Hp => hp.max,
            AttrId::LostHp => (hp.max - hp.current).max(0),
            attr => managers.attribute.get(target_uid, attr),
        }
    };
    let requested = scale_permille(basis, *permille);
    let floor = scale_permille(hp.max, managers.buff.lost_life_floor_permille(target_uid));
    if (behavior.spec.key.opcode, *mode, attr) == (30005, 0, AttrId::Hp)
        && floor == 0
        && hp.current > 0
        && matches!(event, Some(BattleEvent::ToughnessBroken { target_uid: owner, .. }) if *owner == target_uid)
    {
        return Some(requested);
    }
    Some(requested.min((hp.current - floor).max(0)))
}
