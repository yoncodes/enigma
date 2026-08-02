use crate::engine::{
    manager::hp::{DamageEffectKind, HpCommand, HpDamage, HurtInfoData},
    skill::rule::CommandOrigin,
};

use super::{DamageFormulaInput, calculate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageSettlement {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub config_effect: i32,
    pub effect_kind: DamageEffectKind,
    pub hurt: HurtInfoData,
    pub formula: DamageFormulaInput,
}

impl DamageSettlement {
    pub fn command(self) -> Option<HpCommand> {
        let amount = calculate(self.formula);
        (amount > 0).then_some(HpCommand::Damage(HpDamage {
            origin: self.origin,
            source_uid: self.source_uid,
            target_uid: self.target_uid,
            amount,
            config_effect: self.config_effect,
            effect_kind: self.effect_kind,
            assassinate: false,
            ignore_riposte: false,
            hurt: self.hurt,
        }))
    }
}
