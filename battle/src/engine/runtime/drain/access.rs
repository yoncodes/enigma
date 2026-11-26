use super::{BattleManagers, RoundDeterminism, SkillEffectCatalog, TargetContext, TargetPool};

pub(crate) struct BattleAccess<'a> {
    pub(crate) managers: &'a mut BattleManagers,
    pub(crate) pool: &'a TargetPool,
    pub(crate) catalog: &'a SkillEffectCatalog,
    pub(crate) determinism: &'a mut RoundDeterminism,
    pub(crate) context: TargetContext,
}

impl<'a> BattleAccess<'a> {
    pub(crate) fn new(
        managers: &'a mut BattleManagers,
        pool: &'a TargetPool,
        catalog: &'a SkillEffectCatalog,
        determinism: &'a mut RoundDeterminism,
        context: TargetContext,
    ) -> Self {
        Self {
            managers,
            pool,
            catalog,
            determinism,
            context,
        }
    }
}
