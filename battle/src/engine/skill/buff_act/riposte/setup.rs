use crate::engine::skill::{
    behavior::{classify::BehaviorKind, registry},
    buff_act::registry::{BuffActKind, resolve_feature},
    effect::ParsedBehavior,
    rule::{
        SetupStage,
        route::{ConditionDriver, ConditionRoute},
    },
};

pub(crate) fn place_grant(
    db: &config::GameDB,
    behavior: &ParsedBehavior,
    mut route: ConditionRoute,
) -> ConditionRoute {
    let buff_gate = |driver: &Option<ConditionDriver>| {
        matches!(
            driver,
            Some(ConditionDriver::Setup(setup))
                if setup.stage == SetupStage::BuffGate
        )
    };
    if !route
        .branches
        .iter()
        .any(|branch| buff_gate(&branch.driver))
        || !registry::find(behavior)
            .is_some_and(|definition| definition.kind == BehaviorKind::AddBuff)
    {
        return route;
    }
    let Some(buff_id) = behavior.arg(0) else {
        return route;
    };
    let ally_riposte = db.skill_buff.get(buff_id).is_some_and(|buff| {
        buff.features
            .split('|')
            .filter_map(|feature| resolve_feature(Some(db), feature))
            .any(|feature| {
                feature.arguments_supported
                    && feature.kind == Some(BuffActKind::BeatBack)
                    && super::holder_consumed_buff(&feature.args) == Some(buff_id)
            })
    });
    if ally_riposte {
        for branch in &mut route.branches {
            if let Some(ConditionDriver::Setup(setup)) = &mut branch.driver
                && setup.stage == SetupStage::BuffGate
            {
                setup.stage = SetupStage::RoundStart;
                setup.priority = 1;
            }
        }
    }
    route
}
