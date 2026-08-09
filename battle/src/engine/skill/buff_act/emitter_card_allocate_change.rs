use crate::engine::{manager::buff::ActiveBuffFeature, skill::effect::SkillEffectCatalog};

use super::{is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [card_kind, bonus_percent, ranks @ ..]
        if *card_kind == 1
            && *bonus_percent > 0
            && !ranks.is_empty()
            && ranks.iter().all(|rank| (1..=3).contains(rank)))
}

pub fn weight_bonus(
    feature: &ActiveBuffFeature,
    catalog: &SkillEffectCatalog,
    skill_id: i32,
) -> i32 {
    configured_weight_bonus(
        feature,
        catalog,
        crate::catalog::BattleCatalog::try_global(),
        skill_id,
    )
}

pub(crate) fn configured_weight_bonus(
    feature: &ActiveBuffFeature,
    catalog: &SkillEffectCatalog,
    battle_catalog: Option<crate::catalog::BattleCatalog>,
    skill_id: i32,
) -> i32 {
    if !is_kind(feature, BuffActKind::EmitterCardAllocateChange) {
        return 0;
    }
    let [_, card_kind, bonus_percent, ranks @ ..] = feature.values.as_slice() else {
        return 0;
    };
    if (*card_kind == 1 && !catalog.is_attack(skill_id))
        || (!ranks.is_empty()
            && !ranks.contains(
                &battle_catalog
                    .map(|catalog| catalog.skill_rank(skill_id))
                    .unwrap_or_default(),
            ))
    {
        return 0;
    }
    (*bonus_percent).max(0) * feature.amount.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_configured_attack_card_rank_weight_shape() {
        assert!(supports(&[1, 300, 1, 2]));
        assert!(!supports(&[1, 300]));
        assert!(!supports(&[2, 300, 1, 2]));
        assert!(!supports(&[1, 0, 1, 2]));
        assert!(!supports(&[1, 300, 0]));
    }
}
