use sonettobuf::effect_type_enum::EffectType;

use super::super::*;
use crate::engine::skill::buff_act;

buff_act_definitions! {
    (1048, "Radiance") => Radiance, stat_read: OnTrigger,
        runtime: |context| Some(buff_act::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1048, "Radiance"), &[EffectType::Radiance as i32]));
    (1049, "CrystalNotifySelect") => CrystalNotifySelect, effect_time_subscription: false,
        supports: |args| matches!(args, [total, per_crystal, ..]
            if *total > 0 && *per_crystal > 0), state_consumer: true, initial_state: CrystalSelection,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1049, "CrystalNotifySelect"), &[EffectType::None as i32]));
    (10030, "TwinsNotifySelect") => ConduitCardSelection, effect_time_subscription: false,
        supports: buff_act::conduit_select::supports, state_consumer: true, initial_state: ConduitCardSelection,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10030, "TwinsNotifySelect"), &[EffectType::None as i32]));
    (1050, "HeatScaleUseSkill") => HeatScaleUseSkill,
        scoped_runtime: |context| Some(buff_act::heat_scale_use_skill::rule_ops(context.managers, context.catalog, context.subscriber)),
        supports: |_| true, references: references_for_feature, initial_state: HeatScale,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1050, "HeatScaleUseSkill"), &[EffectType::None as i32]));
    (1051, "CrystalAddBuff") => CrystalAddBuff,
        event: EventKind::SkillAction, phase: AfterDamage, timing: AfterSettlement,
        scoped_runtime: |context| buff_act::crystal_add_buff::scoped_rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [buff_id, blue, purple, green_two, green_three, ..]
            if *buff_id > 0 && [blue, purple, green_two, green_three].iter().all(|value| **value >= 0)), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1051, "CrystalAddBuff"), &[EffectType::None as i32]));
    (1052, "HeatScaleTag") => HeatScaleTag,
        effect_time_subscription: false,
        events: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::GaugeChanged],
        setup: [BattleStart(0), OpeningBuffGate(0), RoundTransitionStart(3)], mechanic_setup: [OpeningBuffGate(0)],
        root_mechanic_setup: [RoundTransitionStart(3)],
        independent_setup: [BattleStart(0, 1)],
        publications: [EventKind::GaugeChanged => BeforePublish],
        frame: CausingFrame,
        scoped_runtime: |context| buff_act::heat_scale_tag::rule_ops(context.managers, context.subscriber, context.event?),
        setup_handler: |context| buff_act::heat_scale_tag::setup_rule_ops(context.managers, context.catalog, &context.subscriber.feature, context.subscriber.stage),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1052, "HeatScaleTag"), &[EffectType::None as i32]));
    (1053, "AttrByHeatScale") => AttrByHeatScale, trigger_child_uid: true,
        supports: buff_act::attr_by_heat_scale::supports,
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1053, "AttrByHeatScale"), &[EffectType::None as i32]));
    (1054, "ConsumeBuffLayerCastChannel") => ConsumeBuffLayerCastChannel,
        runtime: |context| buff_act::consume_buff_layer_cast_channel::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: buff_act::consume_buff_layer_cast_channel::supports,
        parser: buff_act::consume_buff_layer_cast_channel::parse,
        references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1054, "ConsumeBuffLayerCastChannel"), &[]));
    (1062, "HeatScaleDecrCounter") => HeatScaleDecrCounter, effect_time_subscription: false,
        supports: |args| matches!(args, [50 | 100]), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1062, "HeatScaleDecrCounter"), &[EffectType::None as i32]));
}
