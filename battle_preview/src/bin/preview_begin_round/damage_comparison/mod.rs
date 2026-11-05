use std::collections::{HashMap, VecDeque};

use sonettobuf::{FightRound, effect_type_enum::EffectType};

use super::nested_steps;

const DAMAGE_EFFECT_TYPES: [i32; 8] = [
    EffectType::Damage as i32,
    EffectType::Crit as i32,
    EffectType::Origindamage as i32,
    EffectType::Origincrit as i32,
    EffectType::Additionaldamage as i32,
    EffectType::Additionaldamagecrit as i32,
    EffectType::Nuodikarandomattack as i32,
    EffectType::Nuodikateamattack as i32,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DamageIdentity {
    skill_id: i32,
    source_uid: i64,
    target_uid: i64,
    effect_type: i32,
    config_effect: i32,
    buff_act_id: i32,
}

fn observations(round: &FightRound) -> Vec<(DamageIdentity, (i32, bool))> {
    nested_steps(round)
        .flat_map(|step| {
            step.act_effect.iter().filter_map(move |effect| {
                let effect_type = effect.effect_type?;
                DAMAGE_EFFECT_TYPES.contains(&effect_type).then_some((
                    DamageIdentity {
                        skill_id: step.act_id.unwrap_or_default(),
                        source_uid: effect
                            .hurt_info
                            .as_ref()
                            .and_then(|info| info.from_uid)
                            .unwrap_or_else(|| step.from_id.unwrap_or_default()),
                        target_uid: effect.target_id.unwrap_or_default(),
                        effect_type,
                        config_effect: effect.config_effect.unwrap_or_default(),
                        buff_act_id: effect.buff_act_id.unwrap_or_default(),
                    },
                    (
                        effect.effect_num.unwrap_or_default(),
                        effect
                            .hurt_info
                            .as_ref()
                            .and_then(|info| info.career_restraint)
                            .unwrap_or_default(),
                    ),
                ))
            })
        })
        .collect()
}

fn lines(generated: &FightRound, captured: &FightRound, verbose: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if verbose {
        for (label, round) in [("generated", generated), ("captured", captured)] {
            for effect in nested_steps(round)
                .flat_map(|step| &step.act_effect)
                .filter(|effect| effect.effect_type == Some(EffectType::Twinsupcount as i32))
            {
                lines.push(format!(
                    "  conduit consumed {label}={} config={}",
                    effect.effect_num.unwrap_or_default(),
                    effect.config_effect.unwrap_or_default()
                ));
            }
        }
    }
    let mut captured = observations(captured).into_iter().fold(
        HashMap::<DamageIdentity, VecDeque<(i32, bool)>>::new(),
        |mut amounts, (identity, observation)| {
            amounts.entry(identity).or_default().push_back(observation);
            amounts
        },
    );
    for (identity, (generated, generated_restrained)) in observations(generated) {
        match captured.get_mut(&identity).and_then(VecDeque::pop_front) {
            Some((captured, captured_restrained)) => {
                let delta = generated - captured;
                let matches = delta == 0 && generated_restrained == captured_restrained;
                if verbose || !matches {
                    lines.push(format!(
                        "  damage parity identity={identity:?} generated={generated} captured={captured} delta={delta} restrained={generated_restrained}/{captured_restrained} earliest_observable_divergence={}",
                        if matches { "none" } else { "final_effect" },
                    ));
                }
            }
            None => lines.push(format!(
                "  damage parity identity={identity:?} generated={generated} captured=missing"
            )),
        }
    }
    for (identity, amounts) in captured {
        for (captured, restrained) in amounts {
            lines.push(format!(
                "  damage parity identity={identity:?} generated=missing captured={captured} restrained={restrained}"
            ));
        }
    }
    lines
}

pub(super) fn report(generated: &FightRound, captured: &FightRound) {
    for line in lines(
        generated,
        captured,
        battle::tooling::damage_tracing_enabled(),
    ) {
        eprintln!("{line}");
    }
}

#[cfg(test)]
mod tests;
