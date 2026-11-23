pub mod card;
pub mod effect;
pub mod step;
pub(crate) mod timeline;

use sonettobuf::ActEffect;

pub(crate) fn normalize_effect_tree(mut effect: ActEffect) -> ActEffect {
    effect.target_id.get_or_insert(0);
    effect.effect_type.get_or_insert(0);
    effect.effect_num.get_or_insert(0);
    effect.config_effect.get_or_insert(0);
    effect.buff_act_id.get_or_insert(0);
    effect.reserve_id.get_or_insert(0);
    effect.team_type.get_or_insert(0);
    effect.effect_num1.get_or_insert(0);

    if let Some(step) = effect.fight_step.as_mut() {
        step.act_type.get_or_insert(0);
        step.from_id.get_or_insert(0);
        step.to_id.get_or_insert(0);
        step.act_id.get_or_insert(0);
        step.card_index.get_or_insert(0);
        step.fake_timeline.get_or_insert(false);
        step.real_skill_type.get_or_insert(0);
        step.real_skin_id.get_or_insert(0);
        step.act_effect = step
            .act_effect
            .drain(..)
            .map(normalize_effect_tree)
            .collect();
    }

    effect
}
