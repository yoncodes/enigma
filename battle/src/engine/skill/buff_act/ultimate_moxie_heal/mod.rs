use crate::engine::{
    damage::{handler::modified_heal, scale_permille},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        hp::{HpCommand, HpHeal, HpHealKind},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [attr, rate] if AttrId::from_raw(*attr) == Some(AttrId::Attack) && *rate > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::AllyAction(action) = event else {
        return Some(Vec::new());
    };
    if !subscriber.owner_alive || !catalog.is_big_skill(action.skill_id) {
        return Some(Vec::new());
    }
    let [_, rate] = subscriber.args.as_slice() else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    let source_uid = subscriber.owner_uid;
    let attack = managers.origin_attribute(source_uid, AttrId::Attack);
    let base = scale_permille(attack, rate.saturating_mul(action.moxie_spent.max(1)));
    Some(
        pool.main_allies(source_uid)
            .iter()
            .filter(|ally| managers.hp.current(ally.uid) > 0)
            .filter_map(|ally| {
                let amount = modified_heal(base, source_uid, ally.uid, managers, &[]);
                (amount > 0).then_some(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
                    HpHeal {
                        origin,
                        source_uid,
                        target_uid: ally.uid,
                        amount,
                        config_effect: 0,
                        kind: HpHealKind::Normal,
                    },
                ))))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
