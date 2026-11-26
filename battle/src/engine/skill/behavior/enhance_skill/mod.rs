use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    skill::{
        action::SkillRateModifier,
        behavior::{
            BehaviorOpContext,
            registry::{BehaviorConfigError, BehaviorHandler},
        },
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

struct Enhancement {
    cost_buff: i32,
    cost: i32,
    reward_buff: i32,
    reward: i32,
    counted_buffs: Vec<i32>,
    damage_per_layer: i32,
    might_per_layer: i32,
}

impl Enhancement {
    fn parse(behavior: &ParsedBehavior) -> Option<Self> {
        if behavior.raw_arity != 8
            || AttrId::from_raw(behavior.arg(6)?)? != AttrId::IncantationSkillUltMightMultiplier
            || behavior
                .raw_args
                .get(4)
                .is_some_and(|ids| ids.split(',').any(|id| id.trim().is_empty()))
        {
            return None;
        }
        let args = Self {
            cost_buff: behavior.arg(0)?,
            cost: behavior.arg(1)?,
            reward_buff: behavior.arg(2)?,
            reward: behavior.arg(3)?,
            counted_buffs: behavior.arg_list(4)?,
            damage_per_layer: behavior.arg(5)?,
            might_per_layer: behavior.arg(7)?,
        };
        let valid_counted_buffs = match args.counted_buffs.as_slice() {
            [buff] => *buff > 0,
            [first, second] => *first > 0 && *second > 0 && first != second,
            _ => false,
        };
        (args.cost_buff > 0
            && args.cost > 0
            && args.reward_buff > 0
            && args.reward >= 0
            && valid_counted_buffs
            && args.damage_per_layer >= 0
            && args.might_per_layer >= 0)
            .then_some(args)
    }

    fn buff_ids(&self) -> impl Iterator<Item = i32> + '_ {
        std::iter::once(self.cost_buff)
            .chain(self.counted_buffs.iter().copied())
            .chain(
                (self.reward > 0 && !self.counted_buffs.contains(&self.reward_buff))
                    .then_some(self.reward_buff),
            )
    }
}

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    Enhancement::parse(behavior).is_some()
}

impl BehaviorHandler for Handler {
    fn validate_config(
        game: &config::GameDB,
        behavior: &ParsedBehavior,
    ) -> Result<(), BehaviorConfigError> {
        let args = Enhancement::parse(behavior).ok_or(BehaviorConfigError::Unsupported)?;
        args.buff_ids()
            .all(|id| game.skill_buff.get(id).is_some())
            .then_some(())
            .ok_or(BehaviorConfigError::MissingBuff)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        let Some(args) = Enhancement::parse(behavior) else {
            return RuleReferences::default();
        };
        RuleReferences {
            buffs: args.buff_ids().collect(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let args = Enhancement::parse(behavior)?;
        let buffs = &context.managers.buff;
        if buffs.buff_id_amount(context.target_uid, args.cost_buff) < args.cost {
            return Some(Vec::new());
        }
        let origin = super::command_origin(behavior)?;
        let mut ops = vec![RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
            BuffConsume {
                origin,
                target_uid: context.target_uid,
                selector: BuffSelector::ExactId(args.cost_buff),
                amount: args.cost,
                depleted: DepletedBuff::Remove,
            },
        )))];
        if args.reward > 0 {
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                BuffGrant {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.source_uid,
                    buff_id: args.reward_buff,
                    amount: Some(args.reward),
                    occurrences: 1,
                    child_uid_reservations: 0,
                },
            ))));
        }

        let layers = args.counted_buffs.iter().fold(0_i32, |total, &buff| {
            total.saturating_add(buffs.buff_id_amount(context.source_uid, buff))
        });
        let damage = layers.saturating_mul(args.damage_per_layer);
        if damage > 0 {
            context.modifiers.rates.push(SkillRateModifier::fixed(
                context.target_uid,
                origin.key.opcode,
                damage,
                true,
            ));
        }
        let might = layers.saturating_mul(args.might_per_layer);
        if might > 0 {
            context
                .modifiers
                .attack_attributes
                .push((AttrId::IncantationSkillUltMightMultiplier, might));
        }
        Some(ops)
    }
}

#[cfg(test)]
mod tests;
