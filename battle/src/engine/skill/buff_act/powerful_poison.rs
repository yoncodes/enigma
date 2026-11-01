use sonettobuf::{BuffActInfo, effect_type_enum::EffectType};
use crate::engine::{
    damage::{DamageFormulaInput, attribute_scaled_damage, calculate_with_trace, handler as damage},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::{BuffCommand, BuffSetState}, hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData}},
    runtime::determinism::RoundDeterminism,
    skill::{rule::{CommandOrigin, output::{BattleCommand, RuleOp}}, subscriber::BuffActSubscriber, target::TargetPool},
};

fn damage_op(managers: &BattleManagers, pool: &TargetPool, random: &mut RoundDeterminism,
    origin: CommandOrigin, source: i64, target: i64, buff_uid: i64, buff_id: i32, base: i32) -> Option<RuleOp> {
    let crit = super::damage_over_time::poison_can_crit(managers, pool, source)
        && random.roll_hidden_crit(buff_id, source, target, damage::crit_chance(source, target, pool, managers));
    let mut formula = DamageFormulaInput::genesis(base, 1000,
        crate::engine::damage::modifiers::periodic_genesis_multiplier(managers, source, target));
    formula.poison_multiplier = Some(super::damage_over_time::poison_damage_multiplier(managers, source, target));
    formula.crit_multiplier = if crit { damage::crit_damage_multiplier(source, target, pool, managers) } else { 1000 };
    formula.is_crit = crit;
    let amount = calculate_with_trace(formula).amount;
    (amount > 0).then(|| RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
        origin, source_uid: source, target_uid: target, amount, config_effect: 0,
        hurt: Some(HurtInfoData { from_uid: source, is_crit: crit, career_restraint: false,
            reduce_hp: 0, effect_id: 0, skill_id: 0, damage_from: HurtDamageFromType::Buff,
            buff_act_id: 1105, buff_uid,
            hurt_effect_type: if crit { EffectType::Origincrit as i32 } else { EffectType::Origindamage as i32 },
            display_amount: Some(amount) }),
    }))))
}

pub fn settlement_ops(managers: &BattleManagers, pool: &TargetPool, random: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber, advance_round: bool) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::PowerfulPoison) { return None; }
    let [base, growth] = subscriber.args.as_slice() else { return None; };
    let snapshot = managers.buff.snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let previous = snapshot.act_info.iter().find(|info| info.act_id == Some(1105))
        .and_then(|info| info.param.first()).copied().unwrap_or(0).max(0);
    let rate = 1000_i32.saturating_add(previous.saturating_mul(*growth));
    let source = if subscriber.source_uid != 0 { subscriber.source_uid } else { subscriber.owner_uid };
    let base_amount = attribute_scaled_damage(managers.origin_attribute(source, AttrId::Attack), *base,
        subscriber.amount.max(1)).saturating_mul(rate) / 1000;
    let origin = super::command_origin(subscriber)?;
    let mut ops = damage_op(managers, pool, random, origin, source, subscriber.owner_uid,
        subscriber.buff_uid, subscriber.buff_id, base_amount).into_iter().collect::<Vec<_>>();
    if advance_round {
        let mut act_info = snapshot.act_info;
        if let Some(info) = act_info.iter_mut().find(|info| info.act_id == Some(1105)) {
            info.param = vec![previous + 1];
        } else {
            act_info.insert(0, BuffActInfo { act_id: Some(1105), param: vec![previous + 1], ..Default::default() });
        }
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(BuffSetState {
            origin, target_uid: subscriber.owner_uid, buff_uid: subscriber.buff_uid,
            ex_info: None, params: None, act_info: Some(act_info),
        }))));
    }
    Some(ops)
}

pub fn expiration_ops(managers: &BattleManagers, pool: &TargetPool, random: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::BuffRemoved(change) = event else { return None; };
    if !change.duration_expired || change.source_uid != subscriber.owner_uid
        || change.act_id != 1105 || change.act_value <= 0 { return Some(Vec::new()); }
    let [rate] = subscriber.args.as_slice() else { return None; };
    let base = attribute_scaled_damage(managers.origin_attribute(subscriber.owner_uid, AttrId::Attack),
        *rate, change.act_value);
    Some(damage_op(managers, pool, random, super::command_origin(subscriber)?, subscriber.owner_uid,
        change.target_uid, change.buff_uid, change.buff_id, base).into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};
    use crate::engine::event::{kind::EventKind, payload::BuffChangeEvent};

    #[test]
    fn powerful_poison_compounds_and_expiration_uses_recorded_rounds() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(10), team_type: Some(1), current_hp: Some(1000),
                attr: Some(HeroAttribute { hp: Some(1000), attack: Some(1000), ..Default::default() }),
                buffs: vec![BuffInfo { uid: Some(21), buff_id: Some(31420009), from_uid: Some(10), ..Default::default() }],
                ..Default::default()
            }], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(-1), team_type: Some(2), current_hp: Some(100000),
                attr: Some(HeroAttribute { hp: Some(100000), ..Default::default() }),
                buffs: vec![BuffInfo { uid: Some(20), buff_id: Some(31420003), from_uid: Some(10),
                    duration: Some(2), layer: Some(2), ..Default::default() }],
                ..Default::default()
            }], ..Default::default() }), ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        assert!(managers.buff.has_buff_act_kind(-1, super::super::registry::BuffActKind::Poison));
        assert_eq!(managers.buff.buff_act_amount(-1, super::super::registry::BuffActKind::Poison), 2);
        let mut random = RoundDeterminism::default();
        let mut bus = crate::engine::event::bus::EventBus::default();
        let feature = managers.buff.active_features_for_owner(&managers.hp, -1).into_iter()
            .find(|f| super::super::is_kind(f, super::super::registry::BuffActKind::PowerfulPoison)).unwrap();
        let subscriber = super::super::subscriber_from_feature(feature, EventKind::RoundEnd).unwrap();
        for expected in [1000, 1250] {
            let ops = settlement_ops(&managers, &pool, &mut random, &subscriber, true).unwrap();
            assert!(matches!(&ops[0], RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))) if loss.amount == expected));
            for op in ops { crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut bus, op).unwrap(); }
        }
        let feature = managers.buff.active_features_for_owner(&managers.hp, 10).into_iter()
            .find(|f| super::super::is_kind(f, super::super::registry::BuffActKind::PowerfulPoisonRemoveDamage)).unwrap();
        let owner = super::super::subscriber_from_feature(feature, EventKind::BuffRemoved).unwrap();
        let event = BattleEvent::BuffRemoved(BuffChangeEvent { source_uid: 10, target_uid: -1,
            buff_uid: 20, buff_id: 31420003, before_amount: 2, after_amount: 0,
            act_id: 1105, act_value: 2, duration_expired: true });
        let ops = expiration_ops(&managers, &pool, &mut random, &owner, &event).unwrap();
        assert!(matches!(&ops[0], RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))) if loss.amount == 7000));
    }
}
