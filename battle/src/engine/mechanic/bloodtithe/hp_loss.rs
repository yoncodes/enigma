use crate::engine::manager::hp::HurtDamageFromType;

pub(super) fn contribution(
    rules: &config::blood_chalice::Rules,
    loss: i32,
    damage_from: HurtDamageFromType,
) -> i32 {
    if damage_from != HurtDamageFromType::Skill {
        return loss;
    }

    let attack_credit = i64::from(loss) * i64::from(rules.attacked_hp_rate) / 1000;
    (i64::from(loss) + attack_credit).min(i64::from(i32::MAX)) as i32
}
