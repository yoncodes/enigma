//! Explicit diagnostic surface for Battle Preview and BattleCheck.

pub mod entity {
    pub use crate::engine::entity::{
        builder::EntityBuilder,
        input::{EquipmentBuildInput, HeroBuildInput},
        stats::{BattleBalance, Stats},
    };
}

pub mod replay {
    pub use crate::engine::{
        fight::rules::OwnedBattleSkill,
        manager::card::{CardOpType, CardSetup},
        runtime::{
            BattleRuntime as ReplayBattle,
            determinism::{
                CrystalReuseChoice, HandRankChoice, MoxieReductionResistanceChoice,
                RoundDeterminism, SkillTargetChoice,
            },
        },
    };

    pub fn seed_observed_redeal(
        runtime: &mut ReplayBattle,
        hand: &[sonettobuf::CardInfo],
    ) -> Result<(), String> {
        runtime.seed_observed_redeal(hand)
    }
}

pub mod scan {
    pub use crate::engine::{
        entity::{destiny::Destiny, passive::Passive},
        manager::buff::BuffPolicy,
        skill::{
            condition::{ConditionTiming, ParsedCondition, ParsedConditionKind, parse_conditions},
            effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectCatalog, SkillEffectSlot},
            rule::route::{ConditionDriver, ConditionRoute, RouteError},
            target::{TargetRequest, is_mapped_target_code},
        },
    };

    pub mod hero_skill {
        pub use crate::engine::entity::skill::{parse_skill_group, split_ids};
    }

    pub mod halo {
        pub use crate::engine::buff::halo::carriers;
    }

    pub mod behavior {
        pub use crate::engine::skill::behavior::{
            has_destination, is_slot_supported, is_supported,
        };

        pub mod classify {
            pub use crate::engine::skill::behavior::classify::BehaviorSpec;
        }

        pub mod registry {
            pub use crate::engine::skill::behavior::registry::find;
        }
    }

    pub mod buff_act {
        pub mod effect_time {
            pub use crate::engine::skill::buff_act::effect_time::{
                BuffActEvent, classify, supports_duration_policy,
            };
        }

        pub mod registry {
            pub use crate::engine::skill::buff_act::registry::{
                BuffActDestination, ParsedBuffAct, destination, destination_for_feature, find,
                owns_duration, resolve_feature, runtime_event,
            };
        }

        pub mod wire {
            pub use crate::engine::skill::buff_act::wire::{WirePhase, find};
        }
    }

    pub mod condition_registry {
        pub use crate::engine::skill::condition::registry::find_key;
    }

    pub mod effect_catalog {
        pub use crate::engine::skill::effect::catalog::global;
    }
}

pub fn damage_tracing_enabled() -> bool {
    crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage)
}

pub fn unstarted_battle(
    catalog: crate::catalog::BattleCatalog,
    fight: sonettobuf::Fight,
) -> crate::Battle {
    crate::Battle::from_runtime(replay::ReplayBattle::new(catalog, fight))
}

pub fn into_battle(runtime: replay::ReplayBattle) -> crate::Battle {
    crate::Battle::from_runtime(runtime)
}

pub fn start_reply(runtime: &replay::ReplayBattle) -> sonettobuf::StartDungeonReply {
    let (fight, round) = runtime.start_state();
    sonettobuf::StartDungeonReply {
        fight: Some(fight.clone()),
        round: round.cloned(),
    }
}

pub fn opening_hand_size(fight: &sonettobuf::Fight) -> usize {
    crate::engine::manager::card::hand_size(fight)
}

pub fn moxie_down_resistance_attribute_id() -> i32 {
    crate::engine::entity::attr::AttrId::MoxieDownResistance.id()
}

pub fn ultimate_ignores_limit(
    catalog: crate::catalog::BattleCatalog,
    fight: &sonettobuf::Fight,
    owner_uid: i64,
    skill_id: i32,
) -> bool {
    let managers = crate::engine::manager::BattleManagers::seeded_with_catalog(catalog, fight);
    crate::engine::mechanic::card::CardMechanic
        .ultimate_ignores_limit(&managers, owner_uid, skill_id)
}

pub fn system_plan_rule_skills(
    tables: &config::GameDB,
    fight: &sonettobuf::Fight,
    plan_id: i32,
) -> Vec<replay::OwnedBattleSkill> {
    crate::tower::system_plan_rule_skills(tables, fight, plan_id)
}

pub fn configured_conduit_skill_ids(
    game_data: &config::GameDB,
    model_id: i32,
    skill_level: i32,
    destiny_stone: i32,
) -> Result<Option<Vec<i32>>, String> {
    crate::catalog::configured_conduit_skill_ids(game_data, model_id, skill_level, destiny_stone)
        .map_err(|error| format!("{error:?}"))
}

pub fn configured_conduit_skill_ids_for_rank(
    game_data: &config::GameDB,
    model_id: i32,
    skill_level: i32,
    destiny_stone: i32,
    destiny_rank: i32,
) -> Result<Option<Vec<i32>>, String> {
    crate::catalog::configured_conduit_skill_ids_for_rank(
        game_data,
        model_id,
        skill_level,
        destiny_stone,
        destiny_rank,
    )
    .map_err(|error| format!("{error:?}"))
}

pub fn configured_trial_skill_ids(trial_id: i32) -> Result<Vec<i32>, String> {
    let (entity, _) = crate::engine::entity::builder::EntityBuilder::configured_trial(
        crate::catalog::BattleCatalog::global(),
        trial_id,
        -1,
        1,
        1,
    )
    .map_err(|error| error.to_string())?;
    Ok(entity
        .skill_group1
        .into_iter()
        .chain(entity.skill_group2)
        .chain(entity.ex_skill.filter(|skill_id| *skill_id > 0))
        .chain(entity.passive_skill)
        .collect())
}

pub fn shell_process_spec(game_data: &'static config::GameDB, buff_id: i32) -> Option<(i32, i32)> {
    let spec = crate::engine::skill::buff_act::shell::resolve_process_spec(
        crate::catalog::BattleCatalog::new(game_data),
        buff_id,
    )?;
    Some((spec.deployed_buff_id, spec.moxie_delta))
}
