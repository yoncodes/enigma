//! Extra stable genesis damage caused by a completed Impromptu attack.
use sonettobuf::effect_type_enum::EffectType;
use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData}},
    skill::{buff_act::{feature_command_origin, is_kind, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp}},
};

pub(crate) fn rule_ops(managers: &BattleManagers, team: i32, emitter_uid: i64,
    skill_id: i32, events: &[BattleEvent]) -> Vec<RuleOp> {
    let damage = events.iter().filter_map(|event| match event {
        BattleEvent::Hit(hit) if hit.source_uid == emitter_uid && hit.skill_id == skill_id
            && hit.damage_from == HurtDamageFromType::Skill => Some(i64::from(hit.resolved_damage.max(0))),
        _ => None,
    }).sum::<i64>();
    if damage == 0 { return Vec::new(); }
    let mut ops = Vec::new();
    for feature in managers.buff.active_features(&managers.hp) {
        if !feature.owner_alive || feature.team_type == team
            || !is_kind(&feature, BuffActKind::EmitterExtraDamage)
            || !events.iter().any(|event| matches!(event,
                BattleEvent::Hit(hit) if hit.source_uid == emitter_uid && hit.skill_id == skill_id
                    && hit.target_uid == feature.owner_uid && hit.damage_from == HurtDamageFromType::Skill))
        { continue; }
        let [_, rate] = feature.values.as_slice() else { continue; };
        let amount = (damage.saturating_mul(i64::from(*rate)) / 1000)
            .clamp(0, i64::from(i32::MAX)) as i32;
        if amount == 0 { continue; }
        let Some(origin) = feature_command_origin(&feature) else { continue; };
        ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
            origin, source_uid: feature.source_uid, target_uid: feature.owner_uid, amount,
            config_effect: 0, hurt: Some(HurtInfoData {
                from_uid: feature.source_uid, is_crit: false, career_restraint: false,
                reduce_hp: 0, effect_id: 0, skill_id: 0, damage_from: HurtDamageFromType::Buff,
                buff_act_id: 1087, buff_uid: feature.buff_uid,
                hurt_effect_type: EffectType::Origindamage as i32, display_amount: Some(amount),
            }),
        }))));
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(BuffConsume {
            origin, target_uid: feature.owner_uid, selector: BuffSelector::Uid(feature.buff_uid),
            amount: 1, depleted: DepletedBuff::Remove,
        }))));
    }
    ops
}
