use crate::engine::{
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        eureka::{EurekaChange, EurekaCommand},
    },
    skill::rule::output::{BattleCommand, RuleOp},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalDamageSpec {
    pub formula: crate::engine::damage::DamageFormula,
    pub rate: i32,
    pub secondary_rate: i32,
    pub extra_rate: i32,
    pub extra_secondary_rate: i32,
    pub temp_buff_id: i32,
    pub remove_buff_id: i32,
    pub credited_source_uid: i64,
    pub extra_eureka_cost: i32,
    pub power_id: i32,
    pub source_count_cost: i32,
}

pub fn uses_costed_lane(extra_skill_kind: i32) -> bool {
    matches!(
        crate::engine::skill::condition::extra::skill_kind_from_is_extra(extra_skill_kind),
        Some(
            crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction
                | crate::engine::skill::condition::extra::ExtraSkillKind::FollowUp
                | crate::engine::skill::condition::extra::ExtraSkillKind::Riposte
        )
    )
}

impl AdditionalDamageSpec {
    pub fn with_rate(mut self, rate: i32) -> Self {
        self.rate = rate;
        self.secondary_rate = rate;
        self.extra_rate = rate;
        self.extra_secondary_rate = rate;
        self
    }

    pub fn rate(self, main_target: bool, extra_action: bool) -> i32 {
        match (main_target, extra_action) {
            (true, true) => self.extra_rate,
            (false, true) => self.extra_secondary_rate,
            (true, false) => self.rate,
            (false, false) => self.secondary_rate,
        }
    }

    pub fn can_apply(self, managers: &BattleManagers, extra_action: bool) -> bool {
        !extra_action
            || self.extra_eureka_cost == 0
            || managers
                .eureka
                .get(self.credited_source_uid, self.power_id)
                .current
                >= self.extra_eureka_cost
    }

    pub fn attack_replacement(self, managers: &BattleManagers) -> Option<super::AttackReplacement> {
        let buff_id = if self.temp_buff_id > 0 {
            self.temp_buff_id
        } else {
            self.remove_buff_id
        };
        (buff_id > 0)
            .then(|| managers.buff.active_features(&managers.hp))?
            .into_iter()
            .filter(|feature| {
                feature.owner_uid == self.credited_source_uid && feature.buff_id == buff_id
            })
            .find_map(|feature| super::attack_replacement_rule(&feature, &managers.hp))
            .filter(|replacement| {
                replacement.formula == crate::engine::damage::DamageFormula::AdditionalDamage
            })
            .map(|replacement| super::AttackReplacement {
                formula: self.formula,
                ..replacement
            })
    }
}

pub fn resolve(feature: &ActiveBuffFeature) -> Option<AdditionalDamageSpec> {
    match super::feature_kind(feature)? {
        super::registry::BuffActKind::CreateAdditionalDamage => {
            super::create_additional_damage::resolve(feature)
        }
        super::registry::BuffActKind::CreateMaxHpAdditionalDamageAndRemove => {
            super::create_max_hp_additional_damage_and_remove::resolve(feature)
        }
        super::registry::BuffActKind::AttrOnlyCalDamageReplaceAttrAdCreator => {
            super::attr_only_cal_damage_replace_attr_ad_creator::additional_damage(feature)
        }
        _ => None,
    }
}

pub fn extra_action_cost_op(
    feature: &ActiveBuffFeature,
    additional: AdditionalDamageSpec,
    extra_action: bool,
) -> Option<RuleOp> {
    if !extra_action || additional.extra_eureka_cost <= 0 || additional.power_id <= 0 {
        return None;
    }
    let origin = super::feature_command_origin(feature)?;
    Some(RuleOp::Command(BattleCommand::Eureka(
        EurekaCommand::Change(EurekaChange {
            origin,
            source_uid: additional.credited_source_uid,
            target_uid: additional.credited_source_uid,
            power_id: additional.power_id,
            delta: -additional.extra_eureka_cost,
            effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
        }),
    )))
}

pub fn configured(
    catalog: crate::catalog::BattleCatalog,
    buff_id: i32,
    owner_uid: i64,
    source_uid: i64,
) -> Option<(ActiveBuffFeature, AdditionalDamageSpec)> {
    catalog.buff_features(buff_id).into_iter().find_map(|row| {
        let feature = ActiveBuffFeature {
            owner_uid,
            source_uid,
            buff_uid: 0,
            buff_id,
            amount: 1,
            team_type: 0,
            owner_alive: true,
            act_type: row.act_type,
            effect_time: row.effect_time,
            effect_condition: row.effect_condition,
            raw: row.raw,
            values: row.values,
        };
        resolve(&feature).map(|spec| (feature, spec))
    })
}

pub fn active_features(
    managers: &BattleManagers,
    source_uid: i64,
) -> Vec<(ActiveBuffFeature, AdditionalDamageSpec)> {
    let resolved = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == source_uid)
        .filter_map(|feature| resolve(&feature).map(|spec| (feature, spec)))
        .collect::<Vec<_>>();

    resolved
        .iter()
        .filter(|(feature, _)| {
            !resolved.iter().any(|(producer, spec)| {
                producer.buff_uid != feature.buff_uid
                    && producer.owner_uid == feature.owner_uid
                    && spec.temp_buff_id == feature.buff_id
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    #[test]
    fn costed_lane_includes_ripostes_without_collapsing_reinforced_skills() {
        use crate::engine::skill::condition::extra::ExtraSkillKind;

        assert!(uses_costed_lane(ExtraSkillKind::ExtraAction.id()));
        assert!(uses_costed_lane(ExtraSkillKind::FollowUp.id()));
        assert!(uses_costed_lane(ExtraSkillKind::Riposte.id()));
        assert!(!uses_costed_lane(ExtraSkillKind::Reinforced.id()));
        assert!(!uses_costed_lane(0));
    }

    #[test]
    fn additional_damage_uses_its_exact_replacement_buff() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(20_000),
                    attr: Some(HeroAttribute {
                        hp: Some(20_000),
                        ..Default::default()
                    }),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(1),
                            buff_id: Some(31260171),
                            from_uid: Some(1),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(2),
                            buff_id: Some(31200171),
                            from_uid: Some(1),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let spec = AdditionalDamageSpec {
            formula: crate::engine::damage::DamageFormula::AdditionalDamage,
            rate: 750,
            secondary_rate: 750,
            extra_rate: 750,
            extra_secondary_rate: 750,
            temp_buff_id: 31260171,
            remove_buff_id: 0,
            credited_source_uid: 1,
            extra_eureka_cost: 0,
            power_id: 0,
            source_count_cost: 0,
        };

        assert_eq!(
            spec.attack_replacement(&managers),
            Some(super::super::AttackReplacement {
                replaced_attr: crate::engine::entity::attr::AttrId::Attack,
                source_attr: crate::engine::entity::attr::AttrId::Hp,
                amount: 4_000,
                formula: crate::engine::damage::DamageFormula::AdditionalDamage,
            })
        );
    }
}
