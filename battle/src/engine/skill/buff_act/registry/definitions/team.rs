use sonettobuf::effect_type_enum::EffectType;

use super::super::*;
use crate::engine::skill::buff_act;

buff_act_definitions! {
    (834, "EachChangeAttr") => EachChangeAttr,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        stat_read: OnGrant,
        transaction: buff_act::each_change_attr::transaction_rule_ops,
        supports: buff_act::each_change_attr::supports,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(834, "EachChangeAttr"), &[EffectType::None as i32]));
    (1131, "EachChangeAttrOneWay") => EachChangeAttrOneWay,
        transactions: [EventKind::BuffAdded, EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        transaction: buff_act::each_change_attr_one_way::transaction_rule_ops,
        expiry_publications: [(buff_act::effect_time::ROUND_START_DURATION, Hp) => AfterPublish],
        supports: buff_act::each_change_attr_one_way::supports,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1131, "EachChangeAttrOneWay"), &[EffectType::None as i32]).with_one_way_attr_max_hp());
    (1162, "EachChangeAttrOneWayAfterAdd") => EachChangeAttrOneWayAfterAdd,
        transactions: [EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        stat_read: OnGrant,
        transaction: buff_act::each_change_attr_one_way_after_add::transaction_rule_ops,
        supports: buff_act::each_change_attr_one_way_after_add::supports,
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1162, "EachChangeAttrOneWayAfterAdd"), &[EffectType::None as i32]).with_one_way_attr_max_hp());
    (836, "ContractCastChannel") => ContractCastChannel,
        transactions: [EventKind::BuffAdded],
        publication: BeforePublish, frame: CausingFrame,
        runtime: |context| buff_act::contract_cast_channel::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        transaction: buff_act::contract_cast_channel::grant_transaction_rule_ops,
        supports: buff_act::contract_cast_channel::supports,
        references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(836, "ContractCastChannel"), &[]));
    (837, "NoneCastChannel") => NoneCastChannel, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(837, "NoneCastChannel"), &[]));
    (838, "CountContinueChannel") => CountContinueChannel,
        event: EventKind::BuffStateChanged,
        runtime: |context| buff_act::count_continue_channel::rule_ops(context.subscriber, context.event?),
        supports: buff_act::count_continue_channel::supports,
        references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(838, "CountContinueChannel"), &[])
            .with_initial_private_state(buff_act::wire::InitialPrivateStateRule::FourthArgument));
    (861, "FixTempAttrByBuffLayer") => FixTempAttrByBuffLayer, stat_read: OnTrigger,
        supports: buff_act::fix_temp_attr_by_buff_layer::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(861, "FixTempAttrByBuffLayer"), &[EffectType::None as i32]));
    (860, "MustCritAndFixTempAttr") => MustCritAndFixTempAttr,
        stat_read: OnTrigger,
        supports: buff_act::must_crit_and_fix_temp_attr::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(860, "MustCritAndFixTempAttr"), &[EffectType::None as i32]));
    (863, "CreateAdditionalDamage") => CreateAdditionalDamage, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(863, "CreateAdditionalDamage"), &[EffectType::None as i32]));
    (865, "AddPassiveSkills") => AddPassiveSkills,
        supports: |args| matches!(args, [skill_id] if *skill_id > 0), references: references_for_feature, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(865, "AddPassiveSkills"), &[EffectType::None as i32]));
    (869, "ShellProcess") => ShellProcess, effect_time_subscription: false,
        events: [EventKind::ShellDeployed, EventKind::ShellRetrieved], frame: CausingFrame,
        runtime: |context| buff_act::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(869, "ShellProcess"), &[EffectType::None as i32]));
    (870, "Shell") => Shell, event: EventKind::BeAttacked, events: [EventKind::DamageShared], frame: CausingFrame, timing: AfterHitShell,
        runtime: |context| buff_act::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(870, "Shell"), &[EffectType::None as i32]));
    (871, "ShellDebuff") => ShellDebuff, event: EventKind::BeAttacked, frame: CausingFrame, timing: BeforeAfterHitObservers,
        runtime: |context| buff_act::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(871, "ShellDebuff"), &[EffectType::None as i32]));
    (872, "ShareHurt") => ShareHurt, effect_time_subscription: false,
        supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(872, "ShareHurt"), &[EffectType::None as i32]));
    (873, "ShellLock") => ShellLock, event: EventKind::ShellRetrieved, frame: CausingFrame,
        runtime: |context| buff_act::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(873, "ShellLock"), &[EffectType::None as i32]));
    (875, "EmitterTag") => EmitterTag,
        events: [EventKind::ActionQueueCommitted, EventKind::ImpromptuResolved],
        transactions: [EventKind::EurekaChanged],
        setup: [BattleStart(0)], independent_setup: [BattleStart(0, 0)],
        frame: CausingFrame,
        scoped_runtime: |context| buff_act::emitter_tag::rule_ops(context.managers, context.subscriber, context.event?),
        transaction: buff_act::emitter_tag::transaction_rule_ops,
        setup_handler: |context| buff_act::emitter_tag::setup_rule_ops(context.managers, &context.subscriber.feature, context.subscriber.stage),
        supports: |_| true, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(875, "EmitterTag"), &[EffectType::Emittertag as i32]));
    (876, "EmitterCareerChange") => EmitterCareerChange,
        supports: |args| matches!(args, [career] if *career > 0), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(876, "EmitterCareerChange"), &[EffectType::Emittercareerchange as i32]));
    (878, "EmitterNumChange") => EmitterNumChange,
        supports: buff_act::emitter_num_change::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(878, "EmitterNumChange"), &[EffectType::Emitternumchange as i32]));
    (879, "EmitterCardAllocateChange") => EmitterCardAllocateChange, effect_time_subscription: false,
        supports: buff_act::emitter_card_allocate_change::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(879, "EmitterCardAllocateChange"), &[EffectType::None as i32]));
    (880, "EmitterDamageUp") => EmitterDamageUp, effect_time_subscription: false, state_consumer: true;
    (897, "RedOrBlueCount") => RedOrBlueCount, source: Owner, actor: Team,
        runtime: |context| buff_act::red_or_blue_count::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::red_or_blue_count::supports,
        wire: (buff_act::wire::BuffActWireDefinition::new(
            DefinitionKey::new(897, "RedOrBlueCount"),
            &[EffectType::Redorbluecount as i32],
            &[],
            &[EffectType::Redorbluecountchange as i32],
        ).with_snapshot_reserve(buff_act::wire::SnapshotReserveRule::ActCommonParamsTail));
    (881, "UseSkillTeamAddEmitterEnergy") => UseSkillTeamAddEmitterEnergy,
        publication: BeforePublish, frame: CausingFrame,
        runtime: |context| buff_act::use_skill_team_add_emitter_energy::rule_ops(context.managers, context.subscriber, context.event?, context.catalog),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(881, "UseSkillTeamAddEmitterEnergy"), &[EffectType::Useskillteamaddemitterenergy as i32]));
    (882, "FixAttrTeamEnergy") => FixAttrTeamEnergy, effect_time_subscription: false,
        stat_read: OnGrant,
        supports: |args| buff_act::fix_attr_team_energy::supports(BuffActKind::FixAttrTeamEnergy, args), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(882, "FixAttrTeamEnergy"), &[EffectType::Fixattrteamenergy as i32]));
    (883, "FixAttrTeamEnergyAndBuff") => FixAttrTeamEnergyAndBuff,
        runtime: |context| buff_act::fix_attr_team_energy::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| buff_act::fix_attr_team_energy::supports(BuffActKind::FixAttrTeamEnergyAndBuff, args), wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(883, "FixAttrTeamEnergyAndBuff"), &[EffectType::Fixattrteamenergyandbuff as i32]));
    (884, "AddToBuffEntity3") => AddBuffAfterAttack,
        event: EventKind::SkillAction, phase: AfterDamage,
        runtime: |context| buff_act::add_buff_after_attack::rule_ops(context.subscriber, context.event?),
        supports: buff_act::add_buff_after_attack::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(884, "AddToBuffEntity3"), &[EffectType::None as i32]));
    (889, "BeAttackByEmitterDamage") => BeAttackByEmitterDamage,
        runtime: |context| buff_act::be_attack_by_emitter_damage::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::be_attack_by_emitter_damage::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(889, "BeAttackByEmitterDamage"), &[EffectType::None as i32]));
    (10010, "BeAttackedFromUseSkill") => BeAttackedFromUseSkill, source: Applier, timing: HpThresholds,
        runtime: |context| buff_act::be_attacked_from_use_skill::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::be_attacked_from_use_skill::supports, references: references_for_feature,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10010, "BeAttackedFromUseSkill"), &[EffectType::None as i32]));
    (891, "AddSplitEmitterNum") => AddSplitEmitterNum,
        supports: buff_act::emitter_num_change::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(891, "AddSplitEmitterNum"), &[EffectType::Addsplitemitternum as i32]));
    (892, "AttackNumSplitEmitterNum") => AttackNumSplitEmitterNum, effect_time_subscription: false,
        supports: buff_act::emitter_num_change::supports_split, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(892, "AttackNumSplitEmitterNum"), &[EffectType::Conditionsplitemitternum as i32]));
    (893, "EmitterEnergyAddBuff") => EmitterEnergyAddBuff,
        event: EventKind::PlayerActionsResolved, frame: IndependentEvent,
        scoped_runtime: |context| buff_act::emitter_energy_add_buff::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::emitter_energy_add_buff::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(893, "EmitterEnergyAddBuff"), &[EffectType::None as i32]));
    (920, "BuffAddActLimit") => BuffAddActLimit, effect_time_subscription: false,
        supports: buff_act::add_action_point::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(920, "BuffAddActLimit"), &[EffectType::None as i32]));
    (922, "AttrAndLayerAttr") => AttrAndLayerAttr, effect_time_subscription: false,
        supports: |args| args.len() >= 3
            && args.len() % 3 == 0
            && args.chunks_exact(3).all(|values|
                crate::engine::entity::attr::AttrId::from_raw(values[0]).is_some()), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(922, "AttrAndLayerAttr"), &[EffectType::None as i32]));
    (923, "AddCardCastChannel") => AddCardCastChannel,
        event: EventKind::ActionQueueCommitted,
        transactions: [EventKind::BuffAdded, EventKind::BuffRemoved],
        publication: AfterPublish,
        runtime: |context| buff_act::add_card_cast_channel::rule_ops(context.managers, context.subscriber, context.event?),
        transaction: buff_act::add_card_cast_channel::transaction_rule_ops,
        supports: buff_act::add_card_cast_channel::supports, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(923, "AddCardCastChannel"), &[]));
    (924, "EmitterRendTarget") => EmitterRendTarget, effect_time_subscription: false,
        supports: buff_act::emitter_rend_target::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(924, "EmitterRendTarget"), &[]));
    (926, "ExPointAddByHit") => ExPointAddByHit, source: Applier,
        runtime: |context| buff_act::ex_point_add_by_hit::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(926, "ExPointAddByHit"), &[EffectType::None as i32]));
    (927, "AddBuffByOtherExSkill") => AddBuffByOtherExSkill,
        transactions: [EventKind::BuffAdded],
        frame: CausingFrame,
        scoped_runtime: buff_act::add_buff_by_other_ex_skill::rule_ops,
        transaction: buff_act::add_buff_by_other_ex_skill::grant_transaction_rule_ops,
        supports: buff_act::add_buff_by_other_ex_skill::supports,
        wire: (buff_act::wire::BuffActWireDefinition::new(DefinitionKey::new(927, "AddBuffByOtherExSkill"), &[], &[], &[]));
    (932, "FixAttrBySubBuffLayer") => FixAttrBySubBuffLayer,
        supports: buff_act::fix_attr_by_sub_buff_layer::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(932, "FixAttrBySubBuffLayer"), &[EffectType::None as i32]));
    (933, "SubBuff") => SubBuff, effect_time_subscription: false,
        supports: |args| matches!(args, [buff_id] if *buff_id > 0), references: references_for_feature, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(933, "SubBuff"), &[EffectType::None as i32]));
    (928, "AddToTarget") => AddToAttackTargets,
        event: EventKind::SkillAction, phase: AfterDamage,
        runtime: |context| buff_act::add_to_target::rule_ops(context.managers, context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(928, "AddToTarget"), &[EffectType::Addtotarget as i32]));
    (929, "AddCardRecordByRound") => AddCardRecordByRound,
        event: EventKind::ActionQueueCommitted, publication: BeforePublish,
        frame: IndependentEvent,
        runtime: |context| buff_act::card_record::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(929, "AddCardRecordByRound"), &[EffectType::Addcardrecordbyround as i32]));
}
