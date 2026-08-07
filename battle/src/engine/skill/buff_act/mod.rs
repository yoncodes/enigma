pub mod absorb_hurt;
pub mod add_action_point;
pub mod add_attr_by_other_buff_layer;
pub mod add_attr_by_special_count;
pub mod add_buff_after_attack;
pub mod add_buff_both;
pub mod add_buff_by_charging_times;
pub mod add_buff_by_other_ex_skill;
pub mod add_buff_to_enter;
pub mod add_card_cast_channel;
pub mod add_sp_temp_card;
pub mod add_to_buff_entity;
pub mod add_to_buff_entity_2;
pub mod add_to_target;
pub mod additional_damage;
pub mod adrenaline_add_card;
pub mod assassination;
pub mod attr;
pub mod attr_and_layer_attr;
pub mod attr_by_damage_type;
pub mod attr_by_heat_scale;
pub mod attr_by_hero_id;
pub mod attr_by_lost_hp;
pub mod attr_by_shield;
pub mod attr_from_entity;
pub mod attr_only_cal_damage_attack;
pub mod attr_only_cal_damage_hp_replace_attack;
pub mod attr_only_cal_damage_replace_attr_ad_creator;
pub mod be_attack_by_emitter_damage;
pub mod big_skill_no_use_action_point;
pub mod blood_pool;
pub mod buff_round_add;
pub mod bullet;
pub mod burn_real_hurt_fix;
pub mod butterfly_record_skill;
pub mod card_record;
pub mod career_ratio_fix;
pub mod career_restraint;
pub mod cast_channel;
pub mod conduit_select;
pub mod control_team_injury_count_round;
pub mod create_additional_damage;
pub mod create_max_hp_additional_damage_and_remove;
pub mod crit_rate_alter2;
pub mod crit_rate_alter_by_other_buff;
pub mod crystal_add_buff;
pub mod cure;
pub mod damage_not_more_than;
pub mod damage_over_time;
pub mod deadly_poison;
pub mod device_cost_reduce;
pub mod disarm;
pub mod disperse_by_tag;
pub mod dodge_spec_skill;
pub mod dot_no_limit;
pub mod dudu_bone_continue_channel;
pub mod each_change_attr;
pub mod effect_time;
pub mod electric_transform;
pub mod emitter_card_allocate_change;
pub mod emitter_career;
pub mod emitter_energy_add_buff;
pub mod emitter_num_change;
pub mod emitter_rend_target;
pub mod emitter_tag;
pub mod ex_point_add_by_hit;
pub mod ex_point_del;
pub mod ex_point_overflow_bank;
pub mod fix_attr_by_sub_buff_layer;
pub mod fix_attr_by_teammate_injury_count;
pub mod fix_attr_team_energy;
pub mod fix_electric_upgrade;
pub mod fix_temp_attr_by_buff_layer;
pub mod fixed_hurt;
pub mod forbid;
pub mod heat_scale_tag;
pub mod heat_scale_use_skill;
pub mod injury_bank;
pub mod life_attack_fix_rate;
pub mod lost_hp_add_extra_blood_pool_value;
pub mod lost_hp_count_add_buff;
pub mod modify_attr_by_buff_layer;
pub mod monitor_continue_channel;
pub mod must_crit_and_fix_temp_attr;
pub mod nuo_di_ka_cast_channel;
pub mod paper_circle_continue_channel;
pub mod petrified;
pub mod raspberry;
pub mod real_damage_kill;
pub mod rebound;
pub mod red_or_blue_count;
pub mod registry;
pub mod revive;
pub mod riposte;
pub mod share_hurt;
pub mod shell;
pub mod shield;
pub mod sleep;
pub mod special_count_cast_channel;
pub mod special_count_continue_channel;
pub mod team_immunity_times;
pub mod team_share_shield;
pub mod toughness;
pub mod transfer_energy_buff;
pub mod use_damage_skill_add_to_target;
pub mod use_skill;
pub mod use_skill_modifier;
pub mod use_skill_team_add_emitter_energy;
pub mod wire;

use crate::engine::{
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent, subscription::SubscriptionKey},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, CommandOrigin},
    },
    skill::{
        rule::{RuleDomain, output::RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn feature_kind(feature: &ActiveBuffFeature) -> Option<registry::BuffActKind> {
    registry::kind(feature.act_id()?, &feature.act_type)
}

pub fn is_kind(feature: &ActiveBuffFeature, kind: registry::BuffActKind) -> bool {
    feature_kind(feature) == Some(kind)
}

pub fn subscriber_kind(subscriber: &BuffActSubscriber) -> Option<registry::BuffActKind> {
    registry::kind(subscriber.key.definition.opcode, &subscriber.act_type)
}

pub fn subscriber_is_kind(subscriber: &BuffActSubscriber, kind: registry::BuffActKind) -> bool {
    subscriber_kind(subscriber) == Some(kind)
}

pub(crate) fn is_primary_team_feature(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    kind: registry::BuffActKind,
) -> bool {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|candidate| candidate.team_type == feature.team_type && is_kind(candidate, kind))
        .min_by_key(|candidate| (candidate.owner_uid, candidate.buff_uid))
        .is_some_and(|primary| {
            (primary.owner_uid, primary.buff_uid) == (feature.owner_uid, feature.buff_uid)
        })
}

pub(crate) fn is_primary_team_subscriber(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    kind: registry::BuffActKind,
) -> bool {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|candidate| candidate.team_type == subscriber.team_type && is_kind(candidate, kind))
        .min_by_key(|candidate| (candidate.owner_uid, candidate.buff_uid))
        .is_some_and(|primary| {
            (primary.owner_uid, primary.buff_uid) == (subscriber.owner_uid, subscriber.buff_uid)
        })
}

fn origin(opcode: i32, type_name: &str) -> Option<CommandOrigin> {
    registry::find(opcode, type_name).map(|definition| CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: definition.key,
    })
}

pub fn command_origin(subscriber: &BuffActSubscriber) -> Option<CommandOrigin> {
    origin(subscriber.key.definition.opcode, &subscriber.act_type)
}

pub fn feature_command_origin(feature: &ActiveBuffFeature) -> Option<CommandOrigin> {
    origin(feature.act_id()?, &feature.act_type)
}

pub fn subscriber_from_feature(
    feature: ActiveBuffFeature,
    event: EventKind,
) -> Option<BuffActSubscriber> {
    let (&act_id, args) = feature.values.split_first()?;
    let definition = registry::find(act_id, &feature.act_type)?;
    Some(BuffActSubscriber {
        owner_uid: feature.owner_uid,
        source_uid: feature.source_uid,
        buff_uid: feature.buff_uid,
        buff_id: feature.buff_id,
        team_type: feature.team_type,
        owner_alive: feature.owner_alive,
        amount: feature.amount,
        key: SubscriptionKey::new(event, definition.key),
        act_type: feature.act_type,
        effect_time: feature.effect_time,
        effect_condition: feature.effect_condition,
        args: args.to_vec(),
        raw: feature.raw,
    })
}

pub fn feature_runtime_frame_scope(
    feature: &ActiveBuffFeature,
) -> Option<registry::RuntimeFrameScope> {
    Some(
        registry::find(feature.act_id()?, &feature.act_type)?
            .runtime
            .frame_scope,
    )
}

pub fn feature_runtime_execution_timing(
    feature: &ActiveBuffFeature,
) -> Option<registry::RuntimeExecutionTiming> {
    Some(
        registry::find(feature.act_id()?, &feature.act_type)?
            .runtime
            .execution_timing,
    )
}

pub fn transaction_publication(
    feature: &ActiveBuffFeature,
    op: &RuleOp,
    event: crate::engine::event::kind::EventKind,
) -> Option<crate::engine::event::subscription::PublicationPhase> {
    use crate::engine::event::subscription::PublicationPhase;

    if matches!(op, RuleOp::BuffFeatureMarker { .. }) {
        return Some(PublicationPhase::AfterPublish);
    }
    Some(registry::runtime_publication(
        feature.act_id()?,
        &feature.act_type,
        event,
    ))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let mut ops = Vec::new();
    for definition in registry::transaction_definitions(event.kind()) {
        ops.extend(definition
            .transaction
            .handler
            .expect("a transaction route has a handler")(
            managers, event
        ));
    }
    with_feature_runtime_markers(ops)
}

fn changed_features(
    event: &BattleEvent,
    kind: registry::BuffActKind,
) -> Vec<(ActiveBuffFeature, i32)> {
    let change = match event {
        BattleEvent::BuffAdded(change)
        | BattleEvent::BuffChanged(change)
        | BattleEvent::BuffRemoved(change) => change,
        _ => return Vec::new(),
    };
    let amount_delta = change.after_amount - change.before_amount;
    crate::engine::manager::buff::BuffManager::configured_features(change.buff_id)
        .into_iter()
        .filter_map(|mut feature| {
            (feature_kind(&feature) == Some(kind)).then(|| {
                feature.owner_uid = change.target_uid;
                feature.source_uid = change.source_uid;
                feature.buff_uid = change.buff_uid;
                feature.amount = change.after_amount;
                (feature, amount_delta)
            })
        })
        .collect()
}

fn attribute_transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
    kind: registry::BuffActKind,
    rule_op: fn(&BattleManagers, &ActiveBuffFeature, i32) -> Option<RuleOp>,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    changed_features(event, kind)
        .into_iter()
        .flat_map(|(feature, amount_delta)| {
            let Some(op) = rule_op(managers, &feature, amount_delta) else {
                return Vec::new();
            };
            let mut ops = vec![(feature.clone(), op)];
            if !matches!(event, BattleEvent::BuffRemoved(_))
                && let Some(wire) =
                    wire::find(feature.act_id().unwrap_or_default(), &feature.act_type)
            {
                let phase = if matches!(event, BattleEvent::BuffAdded(_)) {
                    wire::WirePhase::Add
                } else {
                    wire::WirePhase::Refresh
                };
                ops.extend(wire.markers(phase).iter().map(|effect_type| {
                    (
                        feature.clone(),
                        RuleOp::BuffFeatureMarker {
                            target_uid: feature.owner_uid,
                            effect_type: *effect_type,
                            effect_num: 0,
                            buff_act_id: 0,
                        },
                    )
                }));
            }
            ops
        })
        .collect()
}

fn ex_point_max_transaction_rule_ops(
    _managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    changed_features(event, registry::BuffActKind::ExPointMaxAdd)
        .into_iter()
        .filter_map(|(feature, amount_delta)| {
            let [_, delta, ..] = feature.values.as_slice() else {
                return None;
            };
            let delta = delta.saturating_mul(amount_delta);
            (delta != 0).then(|| {
                (
                    feature.clone(),
                    RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::ExPoint(
                        crate::engine::manager::ex_point::ExPointCommand::ChangeMax(
                            crate::engine::manager::ex_point::ExPointMaxChange {
                                origin: feature_command_origin(&feature)
                                    .expect("a registered feature has an origin"),
                                target_uid: feature.owner_uid,
                                delta,
                            },
                        ),
                    )),
                )
            })
        })
        .collect()
}

fn with_feature_runtime_markers(
    ops: Vec<(ActiveBuffFeature, RuleOp)>,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let mut decorated = Vec::with_capacity(ops.len());
    let mut marked = Vec::<ActiveBuffFeature>::new();
    for (feature, op) in ops {
        let definition = feature
            .act_id()
            .and_then(|act_id| registry::find(act_id, &feature.act_type));
        let marker = definition
            .filter(|_| !marked.contains(&feature))
            .and_then(|definition| {
                let marker = definition.runtime.marker?;
                runtime_marker_op(
                    definition,
                    feature.owner_uid,
                    feature.source_uid,
                    feature.buff_id,
                    None,
                )
                .map(|op| (marker.position, op))
            });
        match marker {
            Some((registry::RuntimeMarkerPosition::BeforeChanges, marker)) => {
                marked.push(feature.clone());
                decorated.push((feature.clone(), marker));
                decorated.push((feature, op));
            }
            Some((registry::RuntimeMarkerPosition::AfterFirstChange, marker)) => {
                marked.push(feature.clone());
                decorated.push((feature.clone(), op));
                decorated.push((feature, marker));
            }
            None => decorated.push((feature, op)),
        }
    }
    decorated
}

fn power_max_transaction_rule_ops(
    _managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    changed_features(event, registry::BuffActKind::PowerMaxAdd)
        .into_iter()
        .filter_map(|(feature, amount_delta)| {
            let [_, power_id, delta] = feature.values.as_slice() else {
                return None;
            };
            let delta = *delta * amount_delta;
            if *power_id <= 0 || delta == 0 {
                return None;
            }
            let op = RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Eureka(
                crate::engine::manager::eureka::EurekaCommand::ChangeMax {
                    origin: feature_command_origin(&feature)?,
                    source_uid: feature.source_uid,
                    target_uid: feature.owner_uid,
                    power_id: *power_id,
                    delta,
                },
            ));
            Some((feature, op))
        })
        .collect()
}

pub fn attack_consumption_rule_ops(
    managers: &BattleManagers,
    source_uid: i64,
    is_big_skill: bool,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == source_uid)
        .filter_map(|feature| {
            let op = match feature_kind(&feature)? {
                registry::BuffActKind::AttrOnlyCalDamageAttack => {
                    if !attr_only_cal_damage_attack::applies_to_skill(&feature, is_big_skill) {
                        return None;
                    }
                    attr_only_cal_damage_attack::consume_rule_op(managers, &feature)?
                }
                registry::BuffActKind::AttrOnlyCalDamageAttackBigSkill if is_big_skill => {
                    attr_only_cal_damage_attack::consume_rule_op(managers, &feature)?
                }
                _ => return None,
            };
            Some((feature, op))
        })
        .collect()
}

pub fn be_attacked_consumption_rule_ops(
    managers: &BattleManagers,
    target_uid: i64,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == target_uid)
        .filter(|feature| {
            feature_kind(feature) == Some(registry::BuffActKind::AttrOnlyCalDamageBeAttacked)
        })
        .filter_map(|feature| {
            attr_only_cal_damage_attack::consume_rule_op(managers, &feature).map(|op| (feature, op))
        })
        .collect()
}

pub fn configured_command_origin(
    act_id: i32,
    expected_kind: registry::BuffActKind,
) -> Option<CommandOrigin> {
    let act_type = &config::try_get()?.buff_act.get(act_id)?.r#type;
    let definition = registry::find(act_id, act_type)?;
    (definition.kind == expected_kind).then_some(CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: definition.key,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActFrameScope {
    CausingFrame,
    SubscriberFrame,
    ActionFrame,
    IndependentEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActFrameSource {
    Counterparty,
    EventTarget,
    Owner,
    Applier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActFrameOwner {
    Subscriber,
    Event,
    UntargetedEvent,
    Command,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffActRuleOp {
    pub op: RuleOp,
    pub scope: BuffActFrameScope,
    pub source: BuffActFrameSource,
    pub group_with_siblings: bool,
    pub frame_owner: BuffActFrameOwner,
}

impl BuffActRuleOp {
    pub fn causing(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::CausingFrame,
            source: BuffActFrameSource::Counterparty,
            group_with_siblings: true,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn subscriber(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::SubscriberFrame,
            source: BuffActFrameSource::Counterparty,
            group_with_siblings: true,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn subscriber_from_owner(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::SubscriberFrame,
            source: BuffActFrameSource::Owner,
            group_with_siblings: true,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn separate_subscriber_from_owner(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::SubscriberFrame,
            source: BuffActFrameSource::Owner,
            group_with_siblings: false,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn subscriber_from_applier(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::SubscriberFrame,
            source: BuffActFrameSource::Applier,
            group_with_siblings: true,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn independent_event(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::IndependentEvent,
            source: BuffActFrameSource::Counterparty,
            group_with_siblings: true,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn separate_independent_event(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::IndependentEvent,
            source: BuffActFrameSource::Counterparty,
            group_with_siblings: false,
            frame_owner: BuffActFrameOwner::Subscriber,
        }
    }

    pub fn separate_independent_command(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::IndependentEvent,
            source: BuffActFrameSource::Counterparty,
            group_with_siblings: false,
            frame_owner: BuffActFrameOwner::Command,
        }
    }

    pub fn event(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::ActionFrame,
            source: BuffActFrameSource::EventTarget,
            group_with_siblings: false,
            frame_owner: BuffActFrameOwner::Event,
        }
    }

    pub fn untargeted_event_from_owner(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::ActionFrame,
            source: BuffActFrameSource::Owner,
            group_with_siblings: false,
            frame_owner: BuffActFrameOwner::UntargetedEvent,
        }
    }

    pub fn grouped_event(op: RuleOp) -> Self {
        Self {
            op,
            scope: BuffActFrameScope::SubscriberFrame,
            source: BuffActFrameSource::EventTarget,
            group_with_siblings: true,
            frame_owner: BuffActFrameOwner::Event,
        }
    }
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &crate::engine::skill::target::TargetPool,
    catalog: &crate::engine::skill::effect::SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    subscriber: &BuffActSubscriber,
    event: Option<&BattleEvent>,
) -> Option<Vec<BuffActRuleOp>> {
    let definition = registry::find(subscriber.key.definition.opcode, &subscriber.act_type)?;
    let mut context = registry::RuntimeContext {
        managers,
        pool,
        catalog,
        determinism,
        subscriber,
        event,
    };
    let ops = if let Some(handler) = definition.runtime.handler {
        let ops = handler(&mut context)?;
        scoped_rule_ops(
            definition.runtime.frame_scope,
            definition.runtime.frame_source,
            ops,
        )
    } else if let Some(handler) = definition.runtime.scoped_handler {
        handler(&mut context)?
    } else {
        let ops = registry::linked_rule_ops(
            subscriber.owner_uid,
            subscriber.key.definition.opcode,
            &subscriber.act_type,
            &subscriber.args,
        )?;
        scoped_rule_ops(
            definition.runtime.frame_scope,
            definition.runtime.frame_source,
            ops,
        )
    };
    Some(with_runtime_marker(definition, subscriber, event, ops))
}

fn with_runtime_marker(
    definition: &registry::BuffActDefinition,
    subscriber: &BuffActSubscriber,
    event: Option<&BattleEvent>,
    mut ops: Vec<BuffActRuleOp>,
) -> Vec<BuffActRuleOp> {
    let Some(marker) = definition
        .runtime
        .marker
        .filter(|_| event.is_some() && !ops.is_empty())
    else {
        return ops;
    };
    let position = marker.position;
    let template = &ops[0];
    let marker_op = BuffActRuleOp {
        op: runtime_marker_op(
            definition,
            subscriber.owner_uid,
            subscriber.source_uid,
            subscriber.buff_id,
            event,
        )
        .expect("the definition has runtime marker metadata"),
        scope: template.scope,
        source: template.source,
        group_with_siblings: true,
        frame_owner: template.frame_owner,
    };
    let index = match position {
        registry::RuntimeMarkerPosition::BeforeChanges => 0,
        registry::RuntimeMarkerPosition::AfterFirstChange => 1,
    };
    ops.insert(index.min(ops.len()), marker_op);
    ops
}

fn runtime_marker_op(
    definition: &registry::BuffActDefinition,
    owner_uid: i64,
    source_uid: i64,
    buff_id: i32,
    event: Option<&BattleEvent>,
) -> Option<RuleOp> {
    let marker = definition.runtime.marker?;
    let target_uid = match marker.target {
        registry::RuntimeMarkerTarget::Owner => owner_uid,
        registry::RuntimeMarkerTarget::Source => source_uid,
        registry::RuntimeMarkerTarget::EventSource => event_source_uid(event?)?,
    };
    let effect_type = marker.effect_type.unwrap_or_else(|| {
        wire::find(definition.key.opcode, definition.key.type_name)
            .and_then(|wire| {
                wire.markers(wire::WirePhase::Static)
                    .first()
                    .or_else(|| wire.markers(wire::WirePhase::Add).first())
            })
            .copied()
            .unwrap_or(sonettobuf::effect_type_enum::EffectType::None as i32)
    });
    Some(RuleOp::BuffFeatureMarker {
        target_uid,
        effect_type,
        effect_num: buff_id,
        buff_act_id: definition.key.opcode,
    })
}

fn event_source_uid(event: &BattleEvent) -> Option<i64> {
    match event {
        BattleEvent::SkillAction(action) => Some(action.source_uid),
        BattleEvent::AllyAction(action) => Some(action.source_uid),
        BattleEvent::Hit(hit) => Some(hit.source_uid),
        BattleEvent::EntityDied(death) => Some(death.source_uid),
        BattleEvent::BuffAdded(change)
        | BattleEvent::BuffChanged(change)
        | BattleEvent::BuffRemoved(change) => Some(change.source_uid),
        BattleEvent::HpLost { source_uid, .. } | BattleEvent::HpHealed { source_uid, .. } => {
            Some(*source_uid)
        }
        BattleEvent::ExPointChanged(change) | BattleEvent::ExPointOverflow(change) => {
            Some(change.source_uid)
        }
        BattleEvent::EurekaChanged(change) => Some(change.source_uid),
        BattleEvent::BuffFeatureTriggered(trigger) => Some(trigger.owner_uid),
        _ => None,
    }
}

fn scoped_rule_ops(
    scope: registry::RuntimeFrameScope,
    source: registry::RuntimeFrameSource,
    ops: Vec<RuleOp>,
) -> Vec<BuffActRuleOp> {
    let wrap = match scope {
        registry::RuntimeFrameScope::CausingFrame => BuffActRuleOp::causing,
        registry::RuntimeFrameScope::SubscriberFrame => BuffActRuleOp::subscriber,
        registry::RuntimeFrameScope::IndependentEvent => BuffActRuleOp::independent_event,
    };
    ops.into_iter()
        .map(wrap)
        .map(|mut op| {
            op.source = match source {
                registry::RuntimeFrameSource::Counterparty => BuffActFrameSource::Counterparty,
                registry::RuntimeFrameSource::Owner => BuffActFrameSource::Owner,
                registry::RuntimeFrameSource::Applier => BuffActFrameSource::Applier,
                registry::RuntimeFrameSource::EventTarget => BuffActFrameSource::EventTarget,
            };
            op
        })
        .collect()
}

pub fn setup_rule_ops(
    managers: &BattleManagers,
    catalog: &crate::engine::skill::effect::SkillEffectCatalog,
    subscriber: &crate::engine::skill::subscriber::BuffActSetupSubscriber,
) -> Option<Vec<RuleOp>> {
    let definition = registry::find(subscriber.key.opcode, subscriber.key.type_name)?;
    definition.setup.handler?(&registry::SetupContext {
        managers,
        catalog,
        subscriber,
    })
}

pub fn attack_attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &crate::engine::manager::buff::BuffManager,
    hp: &crate::engine::manager::hp::HpManager,
) -> i32 {
    attack_attribute_delta_for_skill(feature, attr_id, buffs, hp, false, false)
}

pub fn attack_attribute_delta_for_skill(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &crate::engine::manager::buff::BuffManager,
    hp: &crate::engine::manager::hp::HpManager,
    is_big_skill: bool,
    extra_action: bool,
) -> i32 {
    match feature_kind(feature) {
        Some(
            registry::BuffActKind::AttrOnlyCalDamageAttack
            | registry::BuffActKind::AttrOnlyCalDamageBeAttacked,
        ) => attr_only_cal_damage_attack::attribute_delta(feature, attr_id),
        Some(registry::BuffActKind::AttrOnlyCalDamageAttackBigSkill) if is_big_skill => {
            attr_only_cal_damage_attack::attribute_delta(feature, attr_id)
        }
        Some(registry::BuffActKind::AttrOnlyCalDamageInExtra) if extra_action => {
            attr_only_cal_damage_attack::attribute_delta(feature, attr_id)
        }
        Some(registry::BuffActKind::FixTempAttrByBuffLayer) if extra_action => {
            fix_temp_attr_by_buff_layer::attribute_delta(feature, attr_id, buffs)
        }
        Some(registry::BuffActKind::AttrByShield) => {
            attr_by_shield::attribute_delta(feature, attr_id, hp)
        }
        _ => 0,
    }
}

pub fn calculated_attack_attribute_delta_for_skill(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    attributes: &crate::engine::manager::attribute::AttributeManager,
    buffs: &crate::engine::manager::buff::BuffManager,
    hp: &crate::engine::manager::hp::HpManager,
    is_big_skill: bool,
    extra_action: bool,
) -> i32 {
    if extra_action && feature_kind(feature) == Some(registry::BuffActKind::MustCritAndFixTempAttr)
    {
        return must_crit_and_fix_temp_attr::attribute_delta(
            feature, attr_id, attributes, buffs, hp,
        );
    }
    attack_attribute_delta_for_skill(feature, attr_id, buffs, hp, is_big_skill, extra_action)
}

pub fn dynamic_attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &crate::engine::manager::buff::BuffManager,
    hp: &crate::engine::manager::hp::HpManager,
    include_trigger_history: bool,
) -> i32 {
    match feature_kind(feature) {
        Some(registry::BuffActKind::AddAttrBySpecialCount) => {
            add_attr_by_special_count::attribute_delta(feature, attr_id, buffs)
        }
        Some(registry::BuffActKind::AttrByHeatScale) => {
            attr_by_heat_scale::attribute_delta(feature, attr_id, buffs)
        }
        Some(registry::BuffActKind::AttrByLostHp) => {
            attr_by_lost_hp::attribute_delta(feature, attr_id, hp)
        }
        Some(registry::BuffActKind::FixAttrByTeammateInjuryCountNotReset) => {
            fix_attr_by_teammate_injury_count::attribute_delta(
                feature,
                attr_id,
                buffs,
                include_trigger_history,
            )
        }
        Some(registry::BuffActKind::ModifyAttrByBuffLayer) => {
            modify_attr_by_buff_layer::attribute_delta(feature, attr_id, buffs)
        }
        _ => 0,
    }
}

pub fn target_attack_attribute_delta(
    managers: &BattleManagers,
    target_uid: i64,
    extra_action: bool,
    attr_id: AttrId,
) -> i32 {
    if !extra_action {
        return 0;
    }
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.owner_uid == target_uid)
        .map(|feature| shell::extra_action_attribute_delta(&feature, attr_id))
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackReplacement {
    pub replaced_attr: AttrId,
    pub source_attr: AttrId,
    pub amount: i32,
    pub formula: crate::engine::damage::DamageFormula,
}

pub fn attack_replacement_rule(
    feature: &ActiveBuffFeature,
    hp: &crate::engine::manager::hp::HpManager,
) -> Option<AttackReplacement> {
    let definition = registry::find(feature.act_id()?, &feature.act_type)?;
    (definition.state.attack_replacement?)(feature, hp)
}

pub fn attack_replacement(
    feature: &ActiveBuffFeature,
    hp: &crate::engine::manager::hp::HpManager,
) -> Option<i32> {
    attack_replacement_rule(feature, hp).map(|replacement| replacement.amount)
}

pub fn direct_attack_replacement_rule(
    feature: &ActiveBuffFeature,
    hp: &crate::engine::manager::hp::HpManager,
) -> Option<AttackReplacement> {
    attack_replacement_rule(feature, hp).filter(|replacement| {
        replacement.formula != crate::engine::damage::DamageFormula::AdditionalDamage
    })
}

pub fn skill_rate_bonus(feature: &ActiveBuffFeature) -> i32 {
    match feature_kind(feature) {
        Some(registry::BuffActKind::LifeAttackFixRate) => {
            life_attack_fix_rate::skill_rate_bonus(feature)
        }
        _ => 0,
    }
}

pub fn career_ratio_bonus(feature: &ActiveBuffFeature) -> i32 {
    match feature_kind(feature) {
        Some(registry::BuffActKind::CareerRatioFix) => career_ratio_fix::bonus(feature),
        _ => 0,
    }
}

pub fn forces_career_restraint(feature: &ActiveBuffFeature) -> bool {
    match feature_kind(feature) {
        Some(registry::BuffActKind::CareerRestraint) => career_restraint::active(feature),
        _ => false,
    }
}

pub fn configured_attack_attribute_delta(
    buff_id: i32,
    owner_uid: i64,
    managers: &BattleManagers,
) -> Option<(AttrId, i32)> {
    attr_from_entity::configured_delta(buff_id, owner_uid, managers)
}

#[cfg(test)]
mod test;
