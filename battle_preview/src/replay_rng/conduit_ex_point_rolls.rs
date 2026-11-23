use battle::tooling::{
    conduit_ex_point_overflow_bank_act, conduit_ex_point_type, replay::ConditionRandomChoice,
    scan::SkillEffectCatalog,
};
use sonettobuf::{FightRound, FightStep, effect_type_enum::EffectType};

const PASS: i32 = 0;
const FAIL: i32 = 999;

/// Imports the observed outcomes of random Harmonization grants a Conduit character gives itself.
///
/// Each evaluation point (the opening, then every round change) publishes one source-less step
/// whose children each carry the Harmonization gain (or, at the cap, only the overflow bank
/// marker) of a character whose roll passed; a point without that step is a point where every
/// roll failed.
pub(super) fn choices(
    catalog: &SkillEffectCatalog,
    round: &FightRound,
    opening: bool,
) -> Vec<ConditionRandomChoice> {
    observed_choices(
        &catalog.random_self_conduit_ex_point_skills(),
        round,
        opening,
    )
}

fn observed_choices(
    skills: &[(i32, i32)],
    round: &FightRound,
    opening: bool,
) -> Vec<ConditionRandomChoice> {
    if skills.is_empty() {
        return Vec::new();
    }
    let sources = round
        .ex_point_info
        .iter()
        .filter(|info| info.ex_point_type == Some(conduit_ex_point_type()))
        .filter_map(|info| info.uid)
        .collect::<Vec<_>>();
    let mut points = Vec::<Option<Vec<i64>>>::new();
    if opening {
        points.push(round.fight_step.iter().find_map(granted_uids));
    }
    let mut after_round_change = false;
    for step in &round.fight_step {
        if step
            .act_effect
            .iter()
            .any(|effect| effect.effect_type == Some(EffectType::Changeround as i32))
        {
            points.push(None);
            after_round_change = true;
            continue;
        }
        if after_round_change {
            if let Some(point) = points.last_mut() {
                *point = granted_uids(step);
            }
            after_round_change = false;
        }
    }
    let mut choices = Vec::new();
    for granted in points {
        let granted = granted.unwrap_or_default();
        for &(skill_id, opcode) in skills {
            for &source_uid in &sources {
                choices.push(ConditionRandomChoice {
                    skill_id,
                    source_uid,
                    opcode,
                    roll: if granted.contains(&source_uid) {
                        PASS
                    } else {
                        FAIL
                    },
                });
            }
        }
    }
    choices
}

fn granted_uids(step: &FightStep) -> Option<Vec<i64>> {
    if step.act_effect.is_empty() {
        return None;
    }
    step.act_effect
        .iter()
        .map(|effect| granted_uid(effect.fight_step.as_ref()?))
        .collect()
}

/// A passing character either gains Harmonization or, already at its cap, only banks it.
fn granted_uid(child: &FightStep) -> Option<i64> {
    if child.act_id.unwrap_or_default() != 0 || child.from_id.unwrap_or_default() != 0 {
        return None;
    }
    let mut direct = child
        .act_effect
        .iter()
        .filter(|effect| effect.fight_step.is_none());
    match (direct.next(), direct.next()) {
        (Some(gain), None) => (gain.effect_type == Some(EffectType::Expointchange as i32)
            && gain.effect_num.unwrap_or_default() > 0)
            .then_some(gain.target_id)
            .flatten(),
        (None, None) => banked_uid(child),
        _ => None,
    }
}

fn banked_uid(child: &FightStep) -> Option<i64> {
    let mut markers = child
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref())
        .flat_map(|step| &step.act_effect);
    let marker = markers.next()?;
    (markers.next().is_none()
        && marker.effect_type == Some(EffectType::Buffactinfoupdate as i32)
        && marker.buff_act_info.as_ref().and_then(|info| info.act_id)
            == Some(conduit_ex_point_overflow_bank_act()))
    .then_some(marker.target_id)
    .flatten()
}

#[cfg(test)]
#[path = "conduit_ex_point_rolls/tests.rs"]
mod tests;
