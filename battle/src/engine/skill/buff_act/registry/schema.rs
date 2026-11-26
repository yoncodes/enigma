macro_rules! buff_act_definitions {
    (
        $(
            ($id:expr, $type_name:literal) => $kind:ident
            $(, effect_time_subscription: $effect_time_subscription:expr)?
            $(, event: $event:expr)?
            $(, phase: $phase:ident)?
            $(, events: [$($events:expr),*])?
            $(, transactions: [$($transaction_events:expr),*])?
            $(, transaction_wire_markers: [$($transaction_wire_marker_events:expr),*])?
            $(, setup: [$($stage:ident($priority:expr)),*])?
            $(, side_subscriber_setup: [$($side_subscriber_stage:ident($side_subscriber_priority:expr)),*])?
            $(, mechanic_setup: [$($mechanic_stage:ident($mechanic_priority:expr)),*])?
            $(, root_mechanic_setup: [$($root_mechanic_stage:ident($root_mechanic_priority:expr)),*])?
            $(, independent_setup: [$($independent_stage:ident($independent_priority:expr, $setup_order:expr)),*])?
            $(, publication: $publication:ident)?
            $(, publications: [$($publication_event:expr => $event_phase:ident),*])?
            $(, frame: $frame:ident)?
            $(, source: $source:ident)?
            $(, team: $team:ident)?
            $(, actor: $actor:ident)?
            $(, timing: $timing:ident)?
            $(, timings: [$($timing_event:expr => $event_timing:ident),*])?
            $(, timing_override: $timing_override:expr)?
            $(, multiplicity: $multiplicity:ident)?
            $(, trigger_child_uid: $trigger_child_uid:expr)?
            $(, owns_duration: $owns_duration:expr)?
            $(, non_attack_duration: ($duration_stage:expr, $duration_phase:ident))?
            $(, stat_read: $stat_read:ident)?
            $(, runtime_marker: $marker_position:ident($marker_target:ident $(, $marker_effect_type:expr)?))?
            $(, runtime: $runtime:expr)?
            $(, scoped_runtime: $scoped_runtime:expr)?
            $(, fallible_scoped_runtime: $fallible_scoped_runtime:expr)?
            $(, transaction: $transaction:expr)?
            $(, replacement_publication: $replacement_publication:ident)?
            $(, expiry_publications: [$(($expiry_stage:expr, $expiry_attr:ident) => $expiry_phase:ident),*])?
            $(, setup_handler: $setup_handler:expr)?
            $(, supports: $supports:expr)?
            $(, raw_supports: $raw_supports:expr)?
            $(, parser: $parser:expr)?
            $(, references: $references:expr)?
            $(, attack_replacement: $attack_replacement:expr)?
            $(, hp_loss_state: $hp_loss_state:expr)?
            $(, shield_hit: $shield_time:path => ($shield_hit:expr, $shield_cast_admission:expr))?
            $(, riposte_damage: ($riposte_time:expr, $riposte_condition:expr))?
            $(, reserve_promotion: $reserve_event:path => $reserve_promotion:expr)?
            $(, state_consumer: $state_consumer:expr)?
            $(, initial_state: $initial_state:ident)?
            $(, description_required: $description_required:expr)?
            $(, completion_gap: $completion_gap:literal)?
            $(, wire: ($wire:expr))?
        );*
        $(;)?
    ) => {
        pub(super) const DEFINITIONS: &[BuffActDefinition] = &[
            $(BuffActDefinition {
                key: DefinitionKey::new($id, $type_name),
                kind: BuffActKind::$kind,
                runtime: BuffActRuntimeDefinition {
                    effect_time_subscription: buff_act_definitions!(@effect_time_subscription $($effect_time_subscription)?),
                    event_override: buff_act_definitions!(@event $($event)?),
                    phase_override: buff_act_definitions!(@phase $($phase)?),
                    events: &[$($($events),*)?],
                    publication: buff_act_definitions!(@publication $($publication)?),
                    publications: &[$($(($publication_event, PublicationPhase::$event_phase)),*)?],
                    frame_scope: buff_act_definitions!(@frame $($frame)?),
                    frame_source: buff_act_definitions!(@source $($source)?),
                    team_scope: buff_act_definitions!(@team $($team)?),
                    actor_scope: buff_act_definitions!(@actor $($actor)?),
                    reaction_timing: buff_act_definitions!(@timing $($timing)?),
                    timings: &[$($(($timing_event, ReactionTiming::$event_timing)),*)?],
                    timing_override: buff_act_definitions!(@timing_override $($timing_override)?),
                    event_multiplicity: buff_act_definitions!(@multiplicity $($multiplicity)?),
                    reserves_trigger_child_uid: buff_act_definitions!(@trigger_child_uid $($trigger_child_uid)?),
                    owns_duration: buff_act_definitions!(@owns_duration $($owns_duration)?),
                    non_attack_duration: buff_act_definitions!(@non_attack_duration $($duration_stage => $duration_phase)?),
                    marker: buff_act_definitions!(@runtime_marker $($marker_position($marker_target $(, $marker_effect_type)?))?),
                    handler: buff_act_definitions!(@runtime $($runtime)?),
                    scoped_handler: buff_act_definitions!(@scoped_runtime $($scoped_runtime)?),
                    fallible_scoped_handler: buff_act_definitions!(@fallible_scoped_runtime $($fallible_scoped_runtime)?),
                },
                setup: BuffActSetupDefinition {
                    routes: buff_act_definitions!(@setup [$($($stage($priority)),*)?]),
                    side_subscriber_steps: &[$($((SetupStage::$side_subscriber_stage, $side_subscriber_priority)),*)?],
                    mechanic_steps: &[$($((SetupStage::$mechanic_stage, $mechanic_priority)),*)?],
                    root_mechanic_steps: &[$($((SetupStage::$root_mechanic_stage, $root_mechanic_priority)),*)?],
                    independent_steps: &[$($((SetupStage::$independent_stage, $independent_priority, $setup_order)),*)?],
                    handler: buff_act_definitions!(@setup_handler $($setup_handler)?),
                },
                transaction: BuffActTransactionDefinition {
                    events: &[$($($transaction_events),*)?],
                    wire_marker_events: &[$($($transaction_wire_marker_events),*)?],
                    handler: buff_act_definitions!(@transaction $($transaction)?),
                    replacement_publication: buff_act_definitions!(@replacement_publication $($replacement_publication)?),
                    expiry_publications: &[$($(($expiry_stage, crate::engine::entity::attr::AttrId::$expiry_attr, PublicationPhase::$expiry_phase)),*)?],
                },
                state: BuffActStateDefinition {
                    read_timing: buff_act_definitions!(@stat_read $($stat_read)?),
                    attack_replacement: buff_act_definitions!(@attack_replacement $($attack_replacement)?),
                    hp_loss: buff_act_definitions!(@hp_loss_state $($hp_loss_state)?),
                    shield_hit: buff_act_definitions!(@shield_hit $($shield_time => ($shield_hit, $shield_cast_admission))?),
                    riposte_damage: buff_act_definitions!(@riposte_damage $(($riposte_time, $riposte_condition))?),
                    reserve_promotion: buff_act_definitions!(@reserve_promotion $($reserve_event => $reserve_promotion)?),
                    consumer: buff_act_definitions!(@state_consumer $($state_consumer)?),
                    initial: buff_act_definitions!(@initial_state $($initial_state)?),
                },
                supports: buff_act_definitions!(@supports $($supports)?),
                raw_supports: buff_act_definitions!(@raw_supports $($raw_supports)?),
                parser: buff_act_definitions!(@parser $($parser)?),
                references: buff_act_definitions!(@references $($references)?),
                description_required: buff_act_definitions!(@description_required $($description_required)?),
                completion_gap: buff_act_definitions!(@completion_gap $($completion_gap)?),
                wire: buff_act_definitions!(@wire $($wire)?),
            }),*
        ];
    };
    (@effect_time_subscription $value:expr) => { $value };
    (@effect_time_subscription) => { true };
    (@event $event:expr) => { Some($event) };
    (@event) => { None };
    (@phase $phase:ident) => { Some(crate::engine::skill::action::SkillPhase::$phase) };
    (@phase) => { None };
    (@setup [$($stage:ident($priority:expr)),*]) => {
        &[$((SetupStage::$stage, $priority)),*]
    };
    (@publication $publication:ident) => { PublicationPhase::$publication };
    (@publication) => { PublicationPhase::AfterPublish };
    (@replacement_publication $publication:ident) => { Some(PublicationPhase::$publication) };
    (@replacement_publication) => { None };
    (@frame $frame:ident) => { RuntimeFrameScope::$frame };
    (@frame) => { RuntimeFrameScope::SubscriberFrame };
    (@source $source:ident) => { RuntimeFrameSource::$source };
    (@source) => { RuntimeFrameSource::Counterparty };
    (@team $team:ident) => { RuntimeTeamScope::$team };
    (@team) => { RuntimeTeamScope::Same };
    (@actor $actor:ident) => { RuntimeActorScope::$actor };
    (@actor) => { RuntimeActorScope::Owner };
    (@timing $timing:ident) => { ReactionTiming::$timing };
    (@timing) => { ReactionTiming::Immediate };
    (@timing_override $handler:expr) => { Some($handler) };
    (@timing_override) => { None };
    (@multiplicity $multiplicity:ident) => { RuntimeEventMultiplicity::$multiplicity };
    (@multiplicity) => { RuntimeEventMultiplicity::EveryEvent };
    (@trigger_child_uid $value:expr) => { $value };
    (@trigger_child_uid) => { false };
    (@owns_duration $value:expr) => { $value };
    (@owns_duration) => { false };
    (@non_attack_duration $stage:expr => $phase:ident) => {
        Some(($stage, crate::engine::skill::action::SkillPhase::$phase))
    };
    (@non_attack_duration) => { None };
    (@stat_read $value:ident) => { StatReadTiming::$value };
    (@stat_read) => { StatReadTiming::None };
    (@runtime_marker $position:ident($target:ident $(, $effect_type:expr)?)) => {
        Some(RuntimeMarker {
            position: RuntimeMarkerPosition::$position,
            target: RuntimeMarkerTarget::$target,
            effect_type: buff_act_definitions!(@marker_effect_type $($effect_type)?),
        })
    };
    (@runtime_marker) => { None };
    (@marker_effect_type $effect_type:expr) => { Some($effect_type) };
    (@marker_effect_type) => { None };
    (@runtime $handler:expr) => { Some($handler) };
    (@runtime) => { None };
    (@scoped_runtime $handler:expr) => { Some($handler) };
    (@scoped_runtime) => { None };
    (@fallible_scoped_runtime $handler:expr) => { Some($handler) };
    (@fallible_scoped_runtime) => { None };
    (@transaction $handler:expr) => { Some($handler) };
    (@transaction) => { None };
    (@setup_handler $handler:expr) => { Some($handler) };
    (@setup_handler) => { None };
    (@supports $handler:expr) => { Some($handler) };
    (@supports) => { None };
    (@raw_supports $handler:expr) => { Some($handler) };
    (@raw_supports) => { None };
    (@parser $handler:expr) => { Some($handler) };
    (@parser) => { None };
    (@references $handler:expr) => { $handler };
    (@references) => { empty_references };
    (@description_required $value:expr) => { $value };
    (@description_required) => { false };
    (@completion_gap $gap:literal) => { Some($gap) };
    (@completion_gap) => { None };
    (@attack_replacement $handler:expr) => { Some($handler) };
    (@attack_replacement) => { None };
    (@hp_loss_state $handler:expr) => { Some($handler) };
    (@hp_loss_state) => { None };
    (@shield_hit $time:path => ($handler:expr, $admits_cast:expr)) => { Some(ShieldHitRoute { effect_time: $time, handler: $handler, admits_cast: $admits_cast }) };
    (@shield_hit) => { None };
    (@riposte_damage ($time:expr, $condition:expr)) => { Some(($time, $condition)) };
    (@riposte_damage) => { None };
    (@reserve_promotion $event:path => $plan:expr) => { Some(ReservePromotionRoute { event: $event, plan: $plan }) };
    (@reserve_promotion) => { None };
    (@state_consumer $value:expr) => { $value };
    (@state_consumer) => { false };
    (@initial_state $rule:ident) => { Some(BuffActInitialStateRule::$rule) };
    (@initial_state) => { None };
    (@wire $wire:expr) => { Some($wire) };
    (@wire) => { None };
}
