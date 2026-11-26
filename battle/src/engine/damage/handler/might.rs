use crate::engine::entity::attr::AttrId;

pub(super) fn resolve(
    is_ultimate: bool,
    is_conduit: bool,
    attribute: impl Fn(AttrId) -> i32,
) -> (i32, i32) {
    let conduit = if is_conduit {
        attribute(AttrId::ConduitMight)
    } else {
        0
    };
    let ultimate = attribute(AttrId::UltimateMight);
    let incantation = attribute(AttrId::IncantationMight);

    if is_ultimate {
        let converted = incantation * attribute(AttrId::UltimateIncantationMightMultiplier) / 1000;
        let rate = 1000_i64 + i64::from(attribute(AttrId::UltimateMightMultiplier));
        let precise = (1000_i64 + i64::from(conduit) + i64::from(converted)) * 1000
            + i64::from(ultimate) * rate;
        let precise = precise.clamp(i64::from(i32::MIN) * 1000, i64::from(i32::MAX) * 1000);
        (
            precise.div_euclid(1000) as i32,
            precise.rem_euclid(1000) as i32,
        )
    } else {
        let converted = ultimate * attribute(AttrId::IncantationSkillUltMightMultiplier) / 1000;
        (1000 + conduit + incantation + converted, 0)
    }
}
