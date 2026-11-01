use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
};

// True Guidance specifies a fixed attack bonus from Paper Heron's attack when
// the buff is granted at battle entry. Its legacy configuration uses 205 in a
// position otherwise interpreted as a destination attribute.
pub(crate) fn guidance_snapshot(source_attack: i32, values: &[i32]) -> Option<sonettobuf::BuffActInfo> {
    let [834, 205, 0, 102, rate] = values else { return None; };
    let value = (i64::from(source_attack.max(0)) * i64::from(*rate) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    Some(sonettobuf::BuffActInfo { act_id: Some(834), param: Vec::new(),
        str_param: Some(format!("102#{value}")) })
}

pub(crate) fn guidance_attack_delta(feature: &ActiveBuffFeature, buffs: &crate::engine::manager::buff::BuffManager) -> i32 {
    if feature.buff_id != 31410407 || feature.values.as_slice() != [834, 205, 0, 102, 150] { return 0; }
    buffs.snapshot(feature.owner_uid, feature.buff_uid)
        .and_then(|buff| buff.act_info.into_iter().find(|info| info.act_id == Some(834)))
        .and_then(|info| info.str_param)
        .and_then(|raw| raw.strip_prefix("102#").and_then(|value| value.parse::<i32>().ok()))
        .unwrap_or_default()
}

pub fn max_hp_rule_op(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount_delta: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(feature, BuffActKind::EachChangeAttr) || amount_delta == 0 {
        return None;
    }
    let [_, raw_attr, flat, source_attr, permille] = feature.values.as_slice() else {
        return None;
    };
    if AttrId::from_raw(*raw_attr) != Some(AttrId::Hp)
        || AttrId::from_raw(*source_attr) != Some(AttrId::Hp)
    {
        return None;
    }
    let source_uid = if feature.source_uid != 0 {
        feature.source_uid
    } else {
        feature.owner_uid
    };
    let delta = ((i128::from(managers.hp.max(source_uid)) * i128::from(*permille) / 1000
        + i128::from(*flat))
        * i128::from(amount_delta))
    .clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32;
    (delta != 0).then_some(RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(
        MaxHpAdjust {
            origin: buff_act::feature_command_origin(feature)?,
            source_uid,
            target_uid: feature.owner_uid,
            delta,
        },
    ))))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &crate::engine::event::payload::BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    super::attribute_transaction_rule_ops(
        managers,
        event,
        BuffActKind::EachChangeAttr,
        max_hp_rule_op,
    )
}

#[cfg(test)]
mod guidance_tests {
    use super::*;
    use crate::engine::{manager::buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain}};
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    #[test]
    fn guidance_snapshots_source_attack_and_removes_its_bonus_with_the_buff() {
        crate::test_support::init_config();
        let entity = |uid, attack| FightEntityInfo {
            uid: Some(uid), team_type: Some(1), current_hp: Some(1000),
            attr: Some(HeroAttribute { hp: Some(1000), attack: Some(attack), ..Default::default() }),
            ..Default::default()
        };
        let fight = Fight { attacker: Some(FightTeam { entitys: vec![entity(10, 2000), entity(11, 1000)], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo { uid: Some(-1), team_type: Some(2), current_hp: Some(10000),
                attr: Some(HeroAttribute { hp: Some(10000), ..Default::default() }), ..Default::default() }], ..Default::default() }),
            ..Default::default() };
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let origin = CommandOrigin { domain: RuleDomain::Behavior, key: DefinitionKey::new(1, "AddBuff") };
        let damage = |managers: &BattleManagers| {
            let command = crate::engine::damage::handler::resolve_attack_command(
                &crate::engine::damage::AttackPlan { source_uid: 11, target_uid: -1, skill_id: 1,
                    rate: 1000, rate_terms: vec![], attack_attributes: vec![], career_ratio_bonus: 0,
                    attack_career: None, additional_attack_career: None, force_career_restraint: false,
                    critical_multiplier_remainder: 0, is_conduit: false, is_crit: false, assassinate: false,
                    main_target: true, extra_skill_kind: 0, performs_extra_action: false,
                    additional_enabled: false, additional_is_crit: None },
                crate::engine::damage::handler::DamageRuntime { fight_version: 7, pool: &pool,
                    attributes: &managers.attribute, buffs: &managers.buff, target_buffs: &managers.buff,
                    hp: &managers.hp, fields: None, emitter: None, team_inspiration: 0 }, origin,
            ).unwrap();
            let HpCommand::Damage(hit) = command else { unreachable!() };
            hit.amount
        };
        let base_damage = damage(&managers);
        let change = managers.execute_buff(BuffCommand::Grant(BuffGrant {
            origin, source_uid: 10, target_uid: 11, buff_id: 31410407,
            amount: None, occurrences: 1, child_uid_reservations: 0,
        })).unwrap();
        let buff = &change.change.added.as_ref().unwrap().buff;
        assert!(buff.act_info.iter().any(|info| info.act_id == Some(834) && info.str_param.as_deref() == Some("102#300")));
        assert_eq!(managers.origin_attribute(11, AttrId::Attack), 1300);
        assert_eq!(crate::engine::manager::snapshotted_attribute_delta(&managers.buff, &managers.hp, 11, AttrId::Attack), 300);
        assert!(damage(&managers) > base_damage);
        let buff_uid = buff.uid.unwrap();
        managers.execute_buff(BuffCommand::Remove(BuffRemove { origin, target_uid: 11, selector: BuffRemoveSelector::Uid(buff_uid) })).unwrap();
        assert_eq!(managers.origin_attribute(11, AttrId::Attack), 1000);
        assert_eq!(damage(&managers), base_damage);
    }
}
