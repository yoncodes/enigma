use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        BattleManagers,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{buff_act::registry::BuffActKind, target::TargetPool},
};

pub fn route(managers: &BattleManagers, pool: &TargetPool, command: HpCommand) -> Vec<HpCommand> {
    let HpCommand::Damage(mut damage) = command else {
        return vec![command];
    };
    let absorber = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid != damage.target_uid
                && feature.owner_alive
                && pool
                    .main_allies(damage.target_uid)
                    .iter()
                    .any(|ally| ally.uid == feature.owner_uid)
                && super::is_kind(feature, BuffActKind::AbsorbHurt)
        })
        .find(|feature| {
            super::injury_bank::state(managers, feature.owner_uid)
                .is_some_and(|state| state.current < state.cap)
        });
    let Some(feature) = absorber else {
        return vec![HpCommand::Damage(damage)];
    };
    let [_, rate, ..] = feature.values.as_slice() else {
        return vec![HpCommand::Damage(damage)];
    };
    if !(1..=1000).contains(rate) {
        return vec![HpCommand::Damage(damage)];
    }
    let absorbed = damage.amount * *rate / 1000;
    if absorbed <= 0 {
        return vec![HpCommand::Damage(damage)];
    }
    damage.amount -= absorbed;
    vec![
        HpCommand::Lose(HpLoss {
            origin: damage.origin,
            source_uid: damage.source_uid,
            target_uid: feature.owner_uid,
            amount: absorbed,
            config_effect: damage.config_effect,
            hurt: Some(HurtInfoData {
                from_uid: damage.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::AbsorbHurt,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damagefromabsorb as i32,
                display_amount: None,
            }),
        }),
        HpCommand::Damage(damage),
    ]
}

#[cfg(test)]
mod tests;
