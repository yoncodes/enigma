use crate::engine::{
    damage::scale_permille,
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    skill::{
        behavior::{BehaviorOpContext, command_origin, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(crate) struct Handler;
pub(crate) struct GuardHandler;
pub(crate) struct OverflowHandler;

pub(crate) fn supports(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [0 | 1, raw_attr, permille] if AttrId::from_raw(*raw_attr).is_some() && *permille >= 0
    )
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let loss = Loss::read(&context, behavior)?;
        emit(
            &context,
            behavior,
            loss.limited(),
            context.target.active_skill_id,
        )
    }
}

impl BehaviorHandler for GuardHandler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let loss = Loss::read(&context, behavior)?;
        let own_break = matches!(
            context.event,
            Some(BattleEvent::ToughnessBroken { target_uid, .. }) if *target_uid == context.target_uid
        );
        let amount = if own_break && loss.max_hp_basis && loss.floor == 0 && loss.current > 0 {
            loss.requested
        } else {
            loss.limited()
        };
        emit(&context, behavior, amount, breaking_skill(&context))
    }
}

impl BehaviorHandler for OverflowHandler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let loss = Loss::read(&context, behavior)?;
        emit(&context, behavior, loss.limited(), breaking_skill(&context))
    }
}

struct Loss {
    requested: i32,
    current: i32,
    floor: i32,
    max_hp_basis: bool,
}

impl Loss {
    fn read(context: &BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Self> {
        let [mode, raw_attr, permille] = behavior.args.as_slice() else {
            return None;
        };
        let attr = AttrId::from_raw(*raw_attr)?;
        let target = context.target_uid;
        let managers = context.managers;
        let hp = managers.hp.get(target);
        let basis = if *mode == 1 {
            hp.current
        } else {
            match attr {
                AttrId::CurrentHp => hp.current,
                AttrId::Hp => hp.max,
                AttrId::LostHp => (hp.max - hp.current).max(0),
                attr => managers.attribute.get(target, attr),
            }
        };
        Some(Self {
            requested: scale_permille(basis, *permille),
            current: hp.current,
            floor: scale_permille(hp.max, managers.buff.lost_life_floor_permille(target)),
            max_hp_basis: *mode == 0 && attr == AttrId::Hp,
        })
    }

    fn limited(&self) -> i32 {
        self.requested.min((self.current - self.floor).max(0))
    }
}

fn breaking_skill(context: &BehaviorOpContext<'_>) -> i32 {
    match context.event {
        Some(BattleEvent::ToughnessBroken { skill_id, .. }) => *skill_id,
        _ => context.target.active_skill_id,
    }
}

fn emit(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    amount: i32,
    skill_id: i32,
) -> Option<Vec<RuleOp>> {
    if amount <= 0 {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
        HpLoss {
            origin: command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            amount,
            config_effect: behavior.config_effect,
            hurt: (context.active_skill_id != 0).then_some(HurtInfoData {
                from_uid: context.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: context.active_skill_id,
                skill_id,
                damage_from: HurtDamageFromType::SkillEffect,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            }),
        },
    )))])
}
