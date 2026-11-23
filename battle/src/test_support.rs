use std::path::{Path, PathBuf};

/// Loads the workspace game-data snapshot once for tests that exercise config-backed rules.
pub(crate) fn init_config() {
    let data_dir = std::env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("battle crate must live under the workspace root")
                .join("data/excel2json")
        });
    config::init(
        data_dir
            .to_str()
            .expect("workspace game-data path must be valid UTF-8"),
    )
    .expect("test game data must load");
}

pub(crate) fn game_data() -> &'static config::GameDB {
    init_config();
    config::configs::get()
}

pub(crate) fn find_step(
    step: &sonettobuf::FightStep,
    act_id: i32,
) -> Option<&sonettobuf::FightStep> {
    (step.act_id == Some(act_id)).then_some(step).or_else(|| {
        step.act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
            .find_map(|child| find_step(child, act_id))
    })
}

pub(crate) fn contains_skill(step: &sonettobuf::FightStep, skill_id: i32) -> bool {
    find_step(step, skill_id).is_some()
}

pub(crate) fn contains_effect(
    step: &sonettobuf::FightStep,
    effect_type: i32,
    target_uid: i64,
) -> bool {
    step.act_effect.iter().any(|effect| {
        effect.effect_type == Some(effect_type) && effect.target_id == Some(target_uid)
            || effect
                .fight_step
                .as_ref()
                .is_some_and(|child| contains_effect(child, effect_type, target_uid))
    })
}

pub(crate) fn effects<'a>(
    steps: &'a [sonettobuf::FightStep],
    output: &mut Vec<&'a sonettobuf::ActEffect>,
) {
    for step in steps {
        for effect in &step.act_effect {
            output.push(effect);
            if let Some(child) = &effect.fight_step {
                effects(std::slice::from_ref(child), output);
            }
        }
    }
}

pub(crate) fn frame_contains_skill(
    frame: &crate::engine::runtime::record::SemanticFrame,
    skill_id: i32,
) -> bool {
    use crate::engine::runtime::record::{FrameItem, FrameOwner};
    matches!(frame.owner, FrameOwner::Skill { skill_id: id, .. } if id == skill_id)
        || frame.items.iter().any(
            |item| matches!(item, FrameItem::Child(child) if frame_contains_skill(child, skill_id)),
        )
}
