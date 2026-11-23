use sonettobuf::FightTeam;

use crate::engine::{
    entity::attr::AttrId,
    fight::rules::ATTACKER_SIDE_UID,
    manager::BattleManagers,
    skill::{
        behavior::classify::BehaviorKind,
        buff_act::registry::{self, BuffActKind},
        condition::parse::ParsedConditionKind,
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
        target::{TargetPool, request::ALL_ENEMIES_TARGET_CODE},
    },
};

use super::{card_skill_is_blocked, certain_buffs};

pub(super) fn recommend(
    team: &FightTeam,
    power: i32,
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
) -> Option<i32> {
    if managers.card.redealable_count() == 0 {
        return None;
    }
    let skill = team.skill_infos.iter().find_map(|entry| {
        let skill = entry.skill_id?;
        let effect = catalog.get(skill)?;
        let [slot] = effect.slots.as_slice() else {
            return None;
        };
        (entry.cd.unwrap_or_default() == 0
            && power >= entry.need_power?
            && catalog.issues(skill).is_empty()
            && slot.behavior.spec.kind == BehaviorKind::RedealCardKeepStar2
            && slot
                .conditions
                .iter()
                .all(|condition| matches!(condition.kind, ParsedConditionKind::None(_))))
        .then_some(skill)
    })?;
    let live = pool.runtime_view(managers);
    let enemies = live.field_enemies(ATTACKER_SIDE_UID);
    if enemies.is_empty() {
        return None;
    }

    let available = live.main_allies(ATTACKER_SIDE_UID);
    let cards = managers.card.visible_cards().filter(|card| {
        !card.temp_card.unwrap_or_default()
            && card
                .uid
                .is_some_and(|uid| available.iter().any(|source| source.uid == uid))
    });
    let in_hand: Vec<_> = cards
        .filter_map(|card| card.skill_id)
        .flat_map(|skill| defense_families(skill, managers, catalog))
        .collect();

    available
        .iter()
        .any(|source| {
            source
                .skill_group1
                .iter()
                .chain(&source.skill_group2)
                .copied()
                .filter(|&skill| !card_skill_is_blocked(managers, catalog, source.uid, skill))
                .flat_map(|skill| defense_families(skill, managers, catalog))
                .any(|family| {
                    !in_hand.contains(&family)
                        && enemies.iter().all(|target| {
                            !managers.buff.has_active_buff_id_or_type(target.uid, family)
                        })
                })
        })
        .then_some(skill)
}

fn defense_families(
    skill: i32,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
) -> Vec<i32> {
    if !catalog.issues(skill).is_empty()
        || catalog.logic_target(skill) != ALL_ENEMIES_TARGET_CODE
        || catalog.effect_tag(skill) != SkillEffectTag::Debuff as i32
        || catalog.damage_rate(skill) != 0
        || managers.catalog().skill_is_big(skill)
    {
        return Vec::new();
    }
    certain_buffs(skill, catalog)
        .into_iter()
        .filter(|&buff| {
            managers
                .buff
                .definition_features(buff)
                .iter()
                .any(|feature| {
                    let [opcode, attribute, delta] = feature.values.as_slice() else {
                        return false;
                    };
                    AttrId::from_raw(*attribute) == Some(AttrId::CriticalDef)
                        && *delta < 0
                        && registry::find(*opcode, &feature.act_type)
                            .is_some_and(|definition| definition.kind == BuffActKind::Attr)
                })
        })
        .filter_map(|buff| {
            let family = managers.catalog().buff_type_id(buff);
            (family > 0).then_some(family)
        })
        .collect()
}
