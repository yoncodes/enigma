use crate::engine::{
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        hp::{DamageEffectKind, HpChanges, HurtDamageFromType},
    },
    skill::{
        action::{SkillInvocation, SkillRequest, SkillTarget},
        condition::extra::ExtraSkillKind,
        rule::DefinitionKey,
    },
};

use super::registry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Credit {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub key: DefinitionKey,
    pub team: i32,
    skill_id: i32,
}

impl Credit {
    pub(crate) fn cast(self, attacker: i64) -> SkillInvocation {
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: self.owner_uid,
            skill_id: self.skill_id,
        }
        .into();
        invocation.target = SkillTarget::Explicit(attacker);
        invocation.extra_skill_kind = Some(ExtraSkillKind::Riposte);
        invocation
    }
}

pub(super) fn admits_cast(managers: &BattleManagers, source_uid: i64) -> bool {
    managers.hp.current(source_uid) > 0 && managers.terminal_outcome().is_none()
}

pub(crate) fn credits(managers: &BattleManagers, hit: &HpChanges) -> Vec<Credit> {
    if !hit.publishes_hit
        || hit
            .shield_absorbed
            .is_none_or(|shield| shield.absorbed <= 0)
    {
        return Vec::new();
    }
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter_map(|feature| {
            let definition = registry::find(feature.act_id()?, &feature.act_type)?;
            let route = definition.state.shield_hit?;
            (route.handler)(managers, &feature, hit, definition.key)
        })
        .collect()
}

fn absorbed_by_owner_shield(managers: &BattleManagers, hit: &HpChanges, owner_uid: i64) -> bool {
    hit.shield_absorbed
        .filter(|shield| shield.absorbed > 0)
        .and_then(|shield| {
            let carrier_uid = managers.buff.shield_carrier_uid(shield.target_uid)?;
            managers
                .buff
                .active_for(shield.target_uid)
                .find(|buff| buff.uid == Some(carrier_uid))
        })
        .is_some_and(|carrier| carrier.from_uid == Some(owner_uid))
}

pub(super) fn earn(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    hit: &HpChanges,
    key: DefinitionKey,
) -> Option<Credit> {
    if !feature.owner_alive
        || !hit.damage.is_some_and(|damage| {
            damage.effect_kind != DamageEffectKind::Avoided
                && !damage.ignore_riposte
                && damage.hurt.damage_from == HurtDamageFromType::Skill
        })
        || managers.entity.team_type(hit.target_uid)? != feature.team_type
        || managers.entity.team_type(hit.source_uid)? == feature.team_type
        || !absorbed_by_owner_shield(managers, hit, feature.owner_uid)
    {
        return None;
    }
    let [_, skill_id] = feature.values.as_slice() else {
        return None;
    };
    Some(Credit {
        owner_uid: feature.owner_uid,
        buff_uid: feature.buff_uid,
        buff_id: feature.buff_id,
        key,
        team: feature.team_type,
        skill_id: *skill_id,
    })
}
