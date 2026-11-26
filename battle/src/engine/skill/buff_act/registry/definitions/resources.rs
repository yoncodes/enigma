use sonettobuf::effect_type_enum::EffectType;

use super::super::*;
use crate::engine::skill::buff_act;

buff_act_definitions! {
    (951, "CardNotCalSize") => CardNotCalSize,
        supports: |args| !args.is_empty() && args.iter().all(|skill_id| *skill_id > 0), references: references_for_feature, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(951, "CardNotCalSize"), &[EffectType::None as i32]));
    (1137, "EntityExSkillNotCalSize") => EntityExSkillNotCalSize,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1137, "EntityExSkillNotCalSize"), &[EffectType::None as i32]));
    (953, "BloodPoolTag") => BloodPoolTag,
        events: [EventKind::HpLost, EventKind::GaugeChanged],
        setup: [BattleStart(0), Unconditional(0), RoundStart(-1)], independent_setup: [BattleStart(0, 1), Unconditional(0, 0), RoundStart(-1, 0)],
        publications: [EventKind::GaugeChanged => BeforePublish],
        frame: CausingFrame,
        scoped_runtime: |context| buff_act::blood_pool::tag::rule_ops(context.managers, context.subscriber, context.event?),
        setup_handler: |context| buff_act::blood_pool::tag::setup_rule_ops(context.pool, context.managers, context.catalog, &context.subscriber.feature, context.subscriber.stage),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(953, "BloodPoolTag"), &[EffectType::None as i32]));
    (955, "AttrByShield") => AttrByShield,
        supports: buff_act::attr_by_shield::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(955, "AttrByShield"), &[EffectType::Attr as i32]));
    (1005, "AttrOnlyCalDamageReplaceAttrADCreator") => AttrOnlyCalDamageReplaceAttrAdCreator,
        attack_replacement: buff_act::attr_only_cal_damage_replace_attr_ad_creator::additional_damage_attack_replacement, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1005, "AttrOnlyCalDamageReplaceAttrADCreator"), &[EffectType::None as i32]));
    (1007, "AttrOnlyCalDamageReplaceAttr") => AttrOnlyCalDamageReplaceAttr,
        attack_replacement: buff_act::attr_only_cal_damage_replace_attr_ad_creator::skill_attack_replacement, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1007, "AttrOnlyCalDamageReplaceAttr"), &[EffectType::None as i32]));
    (1009, "BloodValueUseSkill") => BloodValueUseSkill, event: EventKind::GaugeChanged,
        runtime: |context| buff_act::blood_pool::value_use_skill::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: buff_act::blood_pool::value_use_skill::supports, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1009, "BloodValueUseSkill"), &[EffectType::None as i32]));
    (1010, "DyingHealDisperse1") => DyingHealDisperse1,
        runtime: |context| buff_act::revive::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::revive::supports_dying_heal, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1010, "DyingHealDisperse1"), &[EffectType::None as i32]));
    (1011, "CureUpByLostHp") => CureUpByLostHp,
        supports: |args| matches!(args, [base, per_step, max_steps, step_size] if *base >= 0 && *per_step >= 0 && *max_steps >= 0 && *step_size > 0),
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1011, "CureUpByLostHp"), &[EffectType::Cureupbylosthp as i32]));
    (1019, "LostHpCountAddBuff") => LostHpCountAddBuff,
        events: [EventKind::HpLost],
        runtime: |context| buff_act::lost_hp_count_add_buff::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::lost_hp_count_add_buff::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1019, "LostHpCountAddBuff"), &[EffectType::None as i32]));
    (1021, "BloodPoolCountAddExPoint") => BloodPoolCountAddExPoint,
        transactions: [EventKind::SkillAction],
        setup: [RoundStart(4)],
        side_subscriber_setup: [RoundStart(4)],
        timing: AfterAction,
        transaction: buff_act::blood_pool::count_add_ex_point::event_rule_ops,
        setup_handler: |context| buff_act::blood_pool::count_add_ex_point::setup_rule_ops(context.managers, &context.subscriber.feature),
        supports: |args| matches!(args, [threshold, amount] if *threshold > 0 && *amount > 0), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1021, "BloodPoolCountAddExPoint"), &[EffectType::None as i32]));
    (1022, "AttrOnlyCalDamageHpReplaceAttackCalSkillDamage") => AttrOnlyCalDamageHpReplaceAttackCalSkillDamage,
        effect_time_subscription: false,
        supports: buff_act::attr_only_cal_damage_hp_replace_attack::supports,
        attack_replacement: buff_act::attr_only_cal_damage_hp_replace_attack::skill_attack_replacement, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1022, "AttrOnlyCalDamageHpReplaceAttackCalSkillDamage"), &[EffectType::None as i32]));
    (1006, "NuoDiKaCastChannel") => NuoDiKaCastChannel,
        scoped_runtime: |context| buff_act::nuo_di_ka_cast_channel::scoped_rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: buff_act::nuo_di_ka_cast_channel::supports, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1006, "NuoDiKaCastChannel"), &[EffectType::None as i32]).with_pre_add(buff_act::wire::WireEffect { effect_type: EffectType::Nuodikarandomattacknum as i32, effect_num: 0, effect_num1: 1 }));
    (1023, "LostHpAddExtraBloodPoolValue") => LostHpAddExtraBloodPoolValue,
        timing: AfterSettlement,
        runtime: |context| Some(buff_act::lost_hp_add_extra_blood_pool_value::rule_ops(context.managers, context.subscriber)),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1023, "LostHpAddExtraBloodPoolValue"), &[EffectType::None as i32]));
    (1024, "MonitorContinueChannel") => MonitorContinueChannel,
        events: [EventKind::AllyAction], source: Owner, team: Opposing,
        scoped_runtime: |context| buff_act::monitor_continue_channel::scoped_rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| args.get(1).is_some_and(|skill_id| *skill_id > 0), references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1024, "MonitorContinueChannel"), &[EffectType::None as i32]));
    (1025, "LifeAttackFixRate") => LifeAttackFixRate,
        effect_time_subscription: false,
        supports: buff_act::life_attack_fix_rate::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1025, "LifeAttackFixRate"), &[EffectType::None as i32]));
    (1026, "CreateMaxHpAdditionalDamageAndRemove") => CreateMaxHpAdditionalDamageAndRemove, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1026, "CreateMaxHpAdditionalDamageAndRemove"), &[EffectType::None as i32]));
    (1027, "AddBuffByChargingTimes") => AddBuffByChargingTimes,
        runtime: |context| Some(buff_act::add_buff_by_charging_times::rule_ops(context.managers, context.pool, context.subscriber)),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1027, "AddBuffByChargingTimes"), &[EffectType::None as i32]));
    (1028, "RealDamageKill") => RealDamageKill, event: buff_act::real_damage_kill::EVENT,
        runtime: |context| buff_act::real_damage_kill::rule_ops(context.managers, context.pool, context.subscriber),
        supports: buff_act::real_damage_kill::supports, initial_state: SourceAttackThreshold,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1028, "RealDamageKill"), &[]));
    (1148, "HurtToProgress") => HurtToProgress,
        event: EventKind::BeAttacked, publication: BeforePublish, frame: CausingFrame,
        runtime: |context| buff_act::hurt_to_progress::rule_ops(context.subscriber, context.event?),
        supports: buff_act::hurt_to_progress::supports,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1148, "HurtToProgress"), &[EffectType::None as i32]));
    (1146, "FoundationShield") => FoundationShield, effect_time_subscription: false,
        supports: buff_act::foundation::supports_shield, state_consumer: true,
        initial_state: SourceAttack, description_required: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1146, "FoundationShield"),
            &[EffectType::None as i32],
        ).without_initial_state_marker());
    (1147, "FoundationCounter") => FoundationCounter, effect_time_subscription: false,
        supports: buff_act::foundation_counter::supports, riposte_damage: (203, 0), state_consumer: true,
        initial_state: FoundationCounter, description_required: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1147, "FoundationCounter"),
            &[EffectType::None as i32],
        ).without_initial_state_marker());
    (1158, "HedoneHurtCounter") => HedoneHurtCounter, effect_time_subscription: false,
        events: [EventKind::TargetAttacked, EventKind::RoundEndAfterFinalSettlement],
        setup: [EnterBattleStatic(0)],
        side_subscriber_setup: [EnterBattleStatic(0)],
        fallible_scoped_runtime: buff_act::hedone_hurt_counter::scoped_rule_ops,
        setup_handler: |context| buff_act::hedone_hurt_counter::setup_rule_ops(context.managers, &context.subscriber.feature, context.subscriber.stage),
        supports: buff_act::hedone_hurt_counter::supports,
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1158, "HedoneHurtCounter"),
            &[EffectType::None as i32],
        ));
    (1159, "HedoneCard") => HedoneCard,
        fallible_scoped_runtime: buff_act::hedone_card::scoped_rule_ops,
        supports: buff_act::hedone_card::supports,
        references: references_for_feature,
        state_consumer: true,
        initial_state: HedoneEnergy,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1159, "HedoneCard"),
            &[EffectType::None as i32],
        ));
    (1160, "HedoneFixAttr") => HedoneFixAttr, effect_time_subscription: false,
        supports: buff_act::hedone_fix_attr::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1160, "HedoneFixAttr"), &[]));
    (1029, "AddAttrByOtherBuffLayer") => AddAttrByOtherBuffLayer,
        effect_time_subscription: false,
        supports: buff_act::add_attr_by_other_buff_layer::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1029, "AddAttrByOtherBuffLayer"), &[EffectType::Attr as i32]));
    (945, "CritRateAlter2") => CritRateAlter2,
        runtime: |context| buff_act::crit_rate_alter2::supports(&context.subscriber.args).then(Vec::new),
        supports: buff_act::crit_rate_alter2::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(945, "CritRateAlter2"), &[EffectType::None as i32]));
    (1071, "CritRateAlter2") => CritRateAlter2,
        effect_time_subscription: false,
        supports: buff_act::crit_rate_alter2::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1071, "CritRateAlter2"), &[EffectType::None as i32]));
    (1030, "CritRateAlterByOtherBuff") => CritRateAlterByOtherBuff,
        runtime: |context| buff_act::crit_rate_alter_by_other_buff::supports(&context.subscriber.args).then(Vec::new),
        supports: buff_act::crit_rate_alter_by_other_buff::supports, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1030, "CritRateAlterByOtherBuff"), &[EffectType::None as i32]));
    (1003, "SpecialCountContinueChannelBuff") => SpecialCountContinueChannelBuff,
        runtime: |context| Some(buff_act::special_count_continue_channel::rule_ops(context.managers, context.subscriber)),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1003, "SpecialCountContinueChannelBuff"), &[EffectType::None as i32]));
    (1002, "SpecialCountCastChannel") => SpecialCountCastChannel,
        scoped_runtime: |context| buff_act::special_count_cast_channel::scoped_rule_ops(context.subscriber, context.event?, context.catalog),
        supports: |args| matches!(args, [skill_id, ..] if *skill_id > 0),
        references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1002, "SpecialCountCastChannel"), &[]));
    (1004, "AddAttrBySpecialCount") => AddAttrBySpecialCount;
    (1031, "ConsumeBuffAddBuffContinueChannel") => ConsumeBuffAddBuffContinueChannel,
        runtime: |context| buff_act::consume_buff_add_buff_continue_channel::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::consume_buff_add_buff_continue_channel::supports,
        references: references_for_feature,
        initial_state: StringCounter,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1031, "ConsumeBuffAddBuffContinueChannel"), &[])
            .without_initial_state_marker());
    (1032, "FixElectricUpgrade") => FixElectricUpgrade,
        effect_time_subscription: false,
        supports: buff_act::fix_electric_upgrade::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1032, "FixElectricUpgrade"), &[EffectType::None as i32]));
    (1033, "TransferEnergyBuff") => TransferEnergyBuff,
        effect_time_subscription: false, events: [EventKind::ExPointOverflow],
        scoped_runtime: |context| buff_act::transfer_energy_buff::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: buff_act::transfer_energy_buff::supports, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1033, "TransferEnergyBuff"), &[EffectType::None as i32]));
    (1034, "AddBuffToEnter") => AddBuffToEnter,
        effect_time_subscription: false, supports: buff_act::add_buff_to_enter::supports, references: references_for_feature, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1034, "AddBuffToEnter"), &[EffectType::None as i32]));
    (946, "BigSkillNoUseActPoint") => BigSkillNoUseActPoint,
        effect_time_subscription: false, events: [EventKind::AllyAction],
        runtime: |context| buff_act::big_skill_no_use_action_point::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: buff_act::big_skill_no_use_action_point::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(946, "BigSkillNoUseActPoint"), &[EffectType::None as i32]));
    (1036, "AddAttrByOtherBuffLayer") => AddAttrByOtherBuffLayer,
        effect_time_subscription: false,
        supports: buff_act::add_attr_by_other_buff_layer::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1036, "AddAttrByOtherBuffLayer"), &[EffectType::None as i32, EffectType::Attr as i32]));
    (1039, "UseSkillByElectricLvContinueChannel") => UseSkillByElectricLvContinueChannel,
        runtime: |context| buff_act::electric_channel::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: buff_act::electric_channel::supports, references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1039, "UseSkillByElectricLvContinueChannel"), &[EffectType::None as i32]));
    (1141, "AddAttrByOtherBuffLayer") => AddAttrBySourceBuffLayer,
        effect_time_subscription: false,
        supports: buff_act::add_attr_by_other_buff_layer::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1141, "AddAttrByOtherBuffLayer"), &[EffectType::Attr as i32]));
    (1041, "RaspberryBigSkill") => RaspberryBigSkill,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1041, "RaspberryBigSkill"), &[EffectType::None as i32]));
    (1042, "Raspberry") => Raspberry, events: [EventKind::BuffRemoved],
        runtime: |context| buff_act::raspberry::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, parser: buff_act::raspberry::parse_feature,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1042, "Raspberry"), &[EffectType::None as i32]).with_max_hp(1, 1042));
    (1043, "Revive") => Revive,
        runtime: |context| buff_act::revive::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::revive::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1043, "Revive"), &[EffectType::Cure as i32]));
}
