use sonettobuf::effect_type_enum::EffectType;

use super::super::*;
use crate::engine::skill::buff_act;

buff_act_definitions! {
    (1157, "NarcissusAddAttrByPassionEx") => MoxieMightField,
        effect_time_subscription: false, events: [EventKind::UltimateMoxiePaid, EventKind::SkillEffectsPrepared],
        source: Owner, actor: Team,
        scoped_runtime: buff_act::moxie_might_field::rule_ops,
        supports: buff_act::moxie_might_field::supports, state_consumer: true, initial_state: ZeroInteger,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1157, "NarcissusAddAttrByPassionEx"), &[EffectType::None as i32])
            .without_initial_state_marker());
    (1154, "NarcissusHealToCounter") => RippleAccrual,
        effect_time_subscription: false, events: [EventKind::HealResolved], source: Owner,
        scoped_runtime: buff_act::ripple_accrual::rule_ops,
        supports: buff_act::ripple_accrual::supports, raw_supports: buff_act::ripple_accrual::supports_link,
        parser: buff_act::ripple_accrual::parse_args,
        references: buff_act::ripple_accrual::references,
        state_consumer: true, initial_state: RippleCapacity,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1154, "NarcissusHealToCounter"), &[EffectType::None as i32])
            .without_initial_state_marker());
    (1155, "NarcissusRitualHealByPassion") => UltimateMoxieHeal,
        event: EventKind::AllyAction, source: Owner, timing: AfterAction,
        runtime: |context| buff_act::ultimate_moxie_heal::rule_ops(context.managers, context.pool, context.catalog, context.subscriber, context.event?),
        supports: buff_act::ultimate_moxie_heal::supports,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1155, "NarcissusRitualHealByPassion"), &[EffectType::None as i32]));
    (502, "Immunity") => Immunity,
        effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(502, "Immunity"), &[]));
    (1069, "ImmunityTimes") => ImmunityTimes,
        effect_time_subscription: false,
        supports: |args| matches!(args, [status] if *status > 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1069, "ImmunityTimes"), &[EffectType::None as i32]));
    (1070, "HeatScaleAddFix") => HeatScaleAddFix,
        effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1070, "HeatScaleAddFix"), &[EffectType::None as i32]));
    (1075, "CardLimitAdd") => CardLimitAdd, effect_time_subscription: false,
        supports: |args| matches!(args, [delta] if *delta != 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1075, "CardLimitAdd"), &[EffectType::None as i32]));
    (1076, "PerElectricAddBuff") => PerElectricAddBuff, effect_time_subscription: false,
        events: [EventKind::FieldTransferred], frame: CausingFrame,
        runtime: |context| buff_act::per_electric_add_buff::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::per_electric_add_buff::supports,
        references: buff_act::per_electric_add_buff::references,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1076, "PerElectricAddBuff"), &[EffectType::None as i32]));
    (1077, "ConsumeCurBuffAndAuraAddBuff") => ConsumeCurBuffAndAuraAddBuff,
        runtime: |context| buff_act::consume_and_grant::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::consume_and_grant::supports, references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1077, "ConsumeCurBuffAndAuraAddBuff"), &[]));
    (1078, "ChangeEmitterCardEnergyLimit") => ChangeEmitterCardEnergyLimit, effect_time_subscription: false,
        supports: buff_act::emitter_card_energy_limit::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1078, "ChangeEmitterCardEnergyLimit"), &[EffectType::None as i32]));
    (1079, "ChangeEmitterSkill") => ChangeEmitterSkill, effect_time_subscription: false,
        supports: buff_act::emitter_skill::supports, references: references_for_feature, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1079, "ChangeEmitterSkill"), &[EffectType::None as i32]));
    (1080, "EmitterAttackNumLimitFixDamage") => EmitterAttackNumLimitFixDamage,
        supports: buff_act::emitter_num_change::supports_limit, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1080, "EmitterAttackNumLimitFixDamage"), &[EffectType::None as i32]));
    (1081, "EmitterFixSubTargetsDamageReduceRate") => EmitterFixSubTargetsDamageReduceRate,
        supports: buff_act::emitter_num_change::supports_reduction, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1081, "EmitterFixSubTargetsDamageReduceRate"), &[EffectType::None as i32]));
    (1082, "UseSkillConsumeFromAddEmitterEnergy") => UseSkillConsumeFromAddEmitterEnergy,
        publication: BeforePublish, frame: CausingFrame,
        runtime: |context| buff_act::eureka_to_inspiration::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::eureka_to_inspiration::supports,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1082, "UseSkillConsumeFromAddEmitterEnergy"), &[EffectType::None as i32]));
    (1084, "RecordReplaceWqMagicCircle") => RecordReplaceWqMagicCircle,
        event: EventKind::RoundEndAfterFinalSettlement,
        fallible_scoped_runtime: buff_act::record_replace_magic_circle::scoped_rule_ops,
        supports: |args| args.is_empty(),
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1084, "RecordReplaceWqMagicCircle"), &[EffectType::None as i32]));
    (1085, "TeamPerPowerAddBuff") => TeamPerPowerAddBuff,
        frame: CausingFrame,
        runtime: |context| buff_act::team_per_power_add_buff::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: buff_act::team_per_power_add_buff::supports,
        references: buff_act::team_per_power_add_buff::references,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1085, "TeamPerPowerAddBuff"), &[EffectType::None as i32]));
    (1086, "ToothFairyFantasy") => ToothFairyFantasy,
        scoped_runtime: buff_act::fantasy::rule_ops,
        supports: buff_act::fantasy::supports,
        parser: buff_act::fantasy::parse,
        references: buff_act::fantasy::references,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1086, "ToothFairyFantasy"), &[EffectType::None as i32]));
    (1087, "EmitterExtraDamage") => EmitterExtraDamage,
        event: EventKind::SkillAction, phase: HitPassives, source: Applier, actor: OpposingTeam,
        runtime: |context| buff_act::emitter_extra_damage::rule_ops(context.subscriber, context.event?),
        supports: buff_act::emitter_extra_damage::supports,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1087, "EmitterExtraDamage"), &[]));
    (1092, "EmitterChangeDmgType") => EmitterChangeDmgType,
        effect_time_subscription: false,
        supports: buff_act::emitter_damage_type::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1092, "EmitterChangeDmgType"), &[EffectType::None as i32]));
    (1114, "HeatScaleBurnAddFix") => HeatScaleBurnAddFix;
    (1125, "TeamShareShield") => TeamShareShield, effect_time_subscription: false,
        supports: buff_act::team_share_shield::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1125, "TeamShareShield"), &[]));
    (1126, "TeamImmunityTimes") => TeamImmunityTimes,
        effect_time_subscription: false,
        setup: [RoundStart(2)],
        setup_handler: |context| buff_act::team_immunity_times::setup_rule_ops(context.managers, &context.subscriber.feature, context.subscriber.stage),
        supports: buff_act::team_immunity_times::supports, initial_state: SecondArgument,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1126, "TeamImmunityTimes"), &[EffectType::None as i32]));
    (1102, "ToughnessRecover") => ToughnessRecover,
        effect_time_subscription: false, events: [EventKind::ToughnessBroken],
        setup: [RoundStartCondition(100)],
        side_subscriber_setup: [RoundStartCondition(100)],
        source: Owner,
        runtime: |context| buff_act::toughness_recover::on_break(context.managers, context.subscriber, context.event?),
        setup_handler: |context| buff_act::toughness_recover::at_round_start(context.managers, &context.subscriber.feature, context.defender_start_round),
        supports: |args| args == [0],
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1102, "ToughnessRecover"), &[EffectType::None as i32]));
    (1127, "TeamExElectricTransConsumeValueAttr") => TeamExElectricTransConsumeValueAttr,
        effect_time_subscription: false, stat_read: OnGrant,
        supports: buff_act::electric_transform::supports_team_attribute, state_consumer: true, initial_state: GrantValue,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1127, "TeamExElectricTransConsumeValueAttr"), &[]));
    (1128, "RecordTeamExElectricTransConsumeValue") => RecordTeamExElectricTransConsumeValue,
        effect_time_subscription: false,
        supports: buff_act::electric_transform::supports_record, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1128, "RecordTeamExElectricTransConsumeValue"), &[EffectType::None as i32]));
    (1129, "ExtraValueElectricTransform") => ExtraValueElectricTransform,
        effect_time_subscription: false,
        supports: buff_act::electric_transform::supports_extra_value, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1129, "ExtraValueElectricTransform"), &[EffectType::None as i32]));
    (1130, "DeviceCostReduce") => DeviceCostReduce,
        effect_time_subscription: false,
        supports: buff_act::device_cost_reduce::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1130, "DeviceCostReduce"), &[EffectType::None as i32]));
    (1133, "UnnamedAddSpCard") => SovereignCardGeneration,
        fallible_scoped_runtime: buff_act::create_sovereign_card::scoped_rule_ops,
        supports: buff_act::create_sovereign_card::supports,
        references: references_for_feature,
        completion_gap: "only player-hand zero-resource generation is captured; stack consumption follows the description, while its step order, enhancement tie selection, and the stacks spent when the card cannot move the full distance remain unproven",
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1133, "UnnamedAddSpCard"), &[EffectType::None as i32]));
    (1134, "UnnamedUnlockSpCard") => SovereignCardUnlock,
        event: EventKind::SovereignUnlock, source: Owner,
        fallible_scoped_runtime: buff_act::unlock_sovereign_card::rule_ops,
        supports: |args| matches!(args, [31470163]),
        references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1134, "UnnamedUnlockSpCard"), &[EffectType::None as i32]));
    (1135, "UnnamedChargingSpCard") => SovereignCardCharge,
        source: Owner, actor: Team,
        fallible_scoped_runtime: buff_act::charge_sovereign_card::rule_ops,
        raw_supports: |_, raw| raw == "1135#1,3#2,4#5,6#7,8",
        completion_gap: "positive charging of a future queued Sovereign card is unproven; held-card charging is implemented",
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1135, "UnnamedChargingSpCard"), &[EffectType::None as i32]));
    (1136, "PerBrokenAddBuff") => GuardBreakReward,
        events: [EventKind::ToughnessBroken], source: Owner,
        runtime: |context| buff_act::guard_break_reward::rule_ops(context.managers, context.subscriber, context.event?),
        raw_supports: |_, raw| matches!(raw, "1136#31470001#1" | "1136#31470001#2"),
        references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1136, "PerBrokenAddBuff"), &[EffectType::None as i32]));
    (1144, "EntityScale") => EntityScale,
        effect_time_subscription: false,
        supports: |args| args.is_empty(),
        state_consumer: true, initial_state: ZeroInteger,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1144, "EntityScale"), &[EffectType::None as i32]));
    (1139, "MeiLeiErCharge") => BuffOwnedCharge,
        effect_time_subscription: false,
        supports: |args| matches!(args, [trigger, limit, linked_skill]
            if *trigger > 0 && *limit >= *trigger && *linked_skill > 0),
        references: references_for_feature,
        state_consumer: true, initial_state: ZeroInteger,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1139, "MeiLeiErCharge"), &[EffectType::None as i32]));
    (1138, "ReplaceEntitySkillGroup") => ReplaceEntitySkillGroup,
        transactions: [EventKind::BuffAdded, EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        transaction: buff_act::bendith::replace_entity_skill_group_transaction,
        raw_supports: buff_act::bendith::supports_replace_entity_skill_group,
        parser: buff_act::bendith::parse_feature,
        references: buff_act::bendith::references,
        // Unproven split: live Bendith grants carry two NONE records and 1138
        // only ever appears with 1140, so one is assigned to each act. Revisit
        // when a capture grants either act without the other (the 1138-only
        // test buff 115370003 would decide it).
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1138, "ReplaceEntitySkillGroup"), &[EffectType::None as i32]));
    (1140, "SkillNoUseActPoint") => SkillNoUseActPoint,
        effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        // Unproven split with 1138; see the note above.
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1140, "SkillNoUseActPoint"), &[EffectType::None as i32]));
}
