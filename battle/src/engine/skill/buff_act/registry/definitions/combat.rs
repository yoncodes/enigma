use sonettobuf::effect_type_enum::EffectType;

use super::super::*;
use crate::engine::skill::buff_act;

buff_act_definitions! {
    (1153, "NarcissusCounterDamageImmune") => RippleBalance,
        effect_time_subscription: false, supports: buff_act::ripple_balance::supports,
        state_consumer: true, initial_state: ZeroInteger,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1153, "NarcissusCounterDamageImmune"), &[EffectType::None as i32])
            .without_initial_state_marker());
    (1156, "NarcissusPassionOriginHurt") => TidalTempoSettlement,
        source: Applier, runtime_marker: BeforeChanges(Owner),
        runtime: buff_act::tidal_tempo_settlement::rule_ops,
        supports: buff_act::tidal_tempo_settlement::supports,
        references: buff_act::tidal_tempo_settlement::references,
        wire: (buff_act::wire::BuffActWireDefinition::add(
            DefinitionKey::new(1156, "NarcissusPassionOriginHurt"), &[EffectType::None as i32]));
    (703, "ExPointMaxAdd") => ExPointMaxAdd,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        frame: CausingFrame,
        transaction: buff_act::ex_point_max_transaction_rule_ops, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(703, "ExPointMaxAdd"), &[]));
    (701, "CardLevelAdd") => CardLevelAdd,
        runtime: buff_act::card_level_add::rule_ops,
        supports: buff_act::card_level_add::supports,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(701, "CardLevelAdd"), &[EffectType::Cardleveladd as i32]));
    (832, "SpExPointMaxAdd") => SpExPointMaxAdd,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        frame: CausingFrame,
        transaction: buff_act::sp_ex_point_max_transaction_rule_ops,
        supports: |args| match args {
            [max_add] => *max_add > 0,
            [max_add, ultimate_cost_offset] => *max_add > 0 && *ultimate_cost_offset >= 0,
            _ => false,
        },
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(832, "SpExPointMaxAdd"), &[]));
    (833, "TransferAddExPoint") => TransferAddExPoint, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(833, "TransferAddExPoint"), &[]));
    (607, "ExPointCardMove") => ExPointCardMove,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(607, "ExPointCardMove"), &[EffectType::Expointcardmove as i32]));
    (603, "ExPointCantAdd") => ExPointCantAdd,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(603, "ExPointCantAdd"), &[EffectType::Expointcantadd as i32]));
    (604, "BuffRoundAdd") => BuffRoundAdd, effect_time_subscription: false,
        supports: buff_act::buff_round_add::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(604, "BuffRoundAdd"), &[]));
    (608, "BuffRoundAddByBuffTypeId") => BuffRoundAddByBuffTypeId, effect_time_subscription: false,
        supports: buff_act::buff_round_add::supports_type_id, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::new(DefinitionKey::new(608, "BuffRoundAddByBuffTypeId"), &[EffectType::Addbuffroundbytypeid as i32], &[], &[]));
    (605, "ExPointDel") => ExPointDel,
        runtime: |context| buff_act::ex_point_del::rule_ops(context.subscriber),
        supports: buff_act::ex_point_del::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(605, "ExPointDel"), &[]));
    (823, "ExPointAdd") => ExPointAdd, event: EventKind::RoundStartCard, source: Owner,
        runtime_marker: BeforeChanges(Owner),
        runtime: |context| buff_act::round_start_ex_point::rule_ops(context.subscriber, context.event?),
        supports: buff_act::round_start_ex_point::supports,
        wire: (buff_act::wire::BuffActWireDefinition::new(DefinitionKey::new(823, "ExPointAdd"), &[], &[EffectType::Expointadd as i32], &[]));
    (609, "ExPointDel") => ExPointDel,
        runtime: |context| buff_act::ex_point_del::rule_ops(context.subscriber),
        supports: buff_act::ex_point_del::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(609, "ExPointDel"), &[]));
    (804, "DisperseByTag") => DisperseByTag, actor: Team,
        runtime: |context| buff_act::disperse_by_tag::rule_ops(context.subscriber, context.event?),
        supports: buff_act::disperse_by_tag::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(804, "DisperseByTag"), &[]));
    (805, "AddPassiveSkillByLayer") => AddPassiveSkillByLayer,
        effect_time_subscription: false,
        supports: |args| matches!(args, [threshold, skill_id] if *threshold > 0 && *skill_id > 0),
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(805, "AddPassiveSkillByLayer"), &[]));
    (722, "CantGetExskill") => CantGetExskill,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(722, "CantGetExskill"), &[EffectType::Cantgetexskill as i32]));
    (709, "BuffAddAct") => BuffAddAct, effect_time_subscription: false,
        supports: buff_act::add_action_point::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(709, "BuffAddAct"), &[EffectType::Buffaddact as i32]));
    (716, "InjuryAbsorb") => InjuryAbsorb, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if (1..=1000).contains(value)),
        state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(716, "InjuryAbsorb"), &[EffectType::Injuryabsorb as i32]));
    (717, "HealCritFix") => HealCritFix, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value > 0),
        state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(717, "HealCritFix"), &[]));
    (719, "PowerMaxAdd") => PowerMaxAdd, effect_time_subscription: false,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        frame: CausingFrame,
        transaction: buff_act::power_max_transaction_rule_ops,
        supports: |args| matches!(args, [power_id, delta] if *power_id > 0 && *delta != 0), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(719, "PowerMaxAdd"), &[]));
    (720, "MonsterLabel") => MonsterLabel,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(720, "MonsterLabel"), &[EffectType::Monsterlabelbuff as i32]));
    (931, "CycleToSub") => CycleToSub,
        effect_time_subscription: false, supports: buff_act::cycle_to_sub::supports, reserve_promotion: buff_act::cycle_to_sub::plan,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(931, "CycleToSub"), &[EffectType::None as i32])
            .with_untagged_grant_event(buff_act::effect_time::BuffActEvent::CycleSubstitution));
    (721, "DotNoLimit") => DotNoLimit,
        scoped_runtime: |context| buff_act::dot_no_limit::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(721, "DotNoLimit"), &[EffectType::Dot as i32]));
    (743, "ReboundBasedOnDamage") => ReboundBasedOnDamage, source: Owner,
        multiplicity: OncePerBatchTarget,
        runtime_marker: BeforeChanges(EventSource, EffectType::Rebound as i32),
        runtime: |context| buff_act::rebound::damage_based_rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::rebound::supports_damage_based,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(743, "ReboundBasedOnDamage"), &[]));
    (795, "None") => TargetingTag,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(795, "None"), &[EffectType::None as i32]));
    (796, "LockHp") => LockHp,
        effect_time_subscription: false, supports: buff_act::lock_hp::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(796, "LockHp"), &[EffectType::Lockhp as i32]));
    (725, "AddToTarget") => AddToTarget,
        runtime: |context| buff_act::add_to_target::rule_ops(context.managers, context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true;
    (731, "CastChannel") => CastChannel,
        event: EventKind::RoundStart,
        scoped_runtime: |context| buff_act::cast_channel::scoped_rule_ops(context.subscriber, context.event?),
        supports: buff_act::cast_channel::supports, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(731, "CastChannel"), &[]));
    (726, "Burn") => Burn, stat_read: OnTrigger,
        runtime: |context| Some(buff_act::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(726, "Burn"), &[EffectType::Burn as i32]));
    (728, "ChangeRemoveBuffUseSkillParam") => ChangeRemoveBuffUseSkillParam,
        effect_time_subscription: false,
        supports: buff_act::change_remove_buff_use_skill_param::supports,
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(728, "ChangeRemoveBuffUseSkillParam"), &[]));
    (748, "UseDamageSkillAddToTarget") => UseDamageSkillAddToTarget,
        effect_time_subscription: false,
        events: [EventKind::SkillCast, EventKind::SkillEffectStarted],
        scoped_runtime: |context| buff_act::use_damage_skill_add_to_target::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: buff_act::use_damage_skill_add_to_target::supports,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(748, "UseDamageSkillAddToTarget"), &[EffectType::None as i32]));
    (757, "UseSkillLoseHpNotFixed") => UseSkillLoseHpNotFixed,
        runtime: |context| buff_act::use_skill_modifier::loss_rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::use_skill_modifier::supports_loss, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(757, "UseSkillLoseHpNotFixed"), &[EffectType::None as i32]));
    (758, "UseSkillAttrFix") => UseSkillAttrFix, effect_time_subscription: false,
        supports: buff_act::use_skill_modifier::supports_attribute, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(758, "UseSkillAttrFix"), &[EffectType::None as i32]));
    (800, "TeammateInjuryCount") => TeammateInjuryCount,
        actor: Team,
        runtime: |context| buff_act::fix_attr_by_teammate_injury_count::tracker_rule_ops(
            context.subscriber,
            context.event?,
        ),
        supports: |args| args.is_empty(), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(800, "TeammateInjuryCount"), &[EffectType::Teammateinjurycount as i32]));
    (801, "FixAttrByTeammateInjuryCountNotReset") => FixAttrByTeammateInjuryCountNotReset,
        effect_time_subscription: false, stat_read: OnTrigger,
        supports: |args| matches!(args, [attr_id, amount, maximum]
            if crate::engine::entity::attr::AttrId::from_raw(*attr_id).is_some()
                && *amount != 0 && *maximum > 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(801, "FixAttrByTeammateInjuryCountNotReset"), &[EffectType::None as i32]));
    (802, "BeatBackByCounter") => BeatBackByCounter, effect_time_subscription: false,
        supports: |args| matches!(args, [skill_id] if *skill_id > 0),
        references: references_for_feature,
        shield_hit: buff_act::effect_time::RIPOSTE => (buff_act::shield_counter::earn, buff_act::shield_counter::admits_cast),
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(802, "BeatBackByCounter"), &[EffectType::None as i32]));
    (803, "Poison") => Poison, stat_read: OnGrant,
        runtime: |context| Some(buff_act::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(803, "Poison"), &[EffectType::Poison as i32]));
    (812, "PoisonSettleCanCrit") => PoisonSettleCanCrit,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(812, "PoisonSettleCanCrit"), &[EffectType::Poisonsettlecancrit as i32]));
    (846, "DuduBoneContinueChannel") => DuduBoneContinueChannel,
        runtime_marker: BeforeChanges(Owner),
        runtime: |context| buff_act::dudu_bone_continue_channel::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| matches!(args, [buff_id, compound_cap, first_stacks, ending_skill, target_code]
            if *buff_id > 0 && *compound_cap >= 0 && *first_stacks > 0
                && *ending_skill > 0 && *target_code != 0), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(846, "DuduBoneContinueChannel"), &[EffectType::Dudubonecontinuechannel as i32]));
    (862, "PaperCircleContinueChannel") => PaperCircleContinueChannel,
        setup: [RoundStart(2)],
        scoped_runtime: |context| buff_act::paper_circle_continue_channel::scoped_rule_ops(context.managers, context.subscriber, context.event?),
        setup_handler: |context| buff_act::paper_circle_continue_channel::setup_rule_ops(context.managers, &context.subscriber.feature),
        supports: |args| matches!(args, [skill_id, _, _, pairs @ ..]
            if *skill_id > 0 && pairs.len() >= 2 && pairs.len() % 2 == 0), references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(862, "PaperCircleContinueChannel"), &[EffectType::None as i32]));
    (850, "AddBuffBoth") => AddBuffBoth,
        runtime: |context| buff_act::add_buff_both::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |args| matches!(args, [enemy_buff_id, ally_target, ally_buff_id]
            if *enemy_buff_id > 0 && *ally_target > 0 && *ally_buff_id > 0), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(850, "AddBuffBoth"), &[EffectType::None as i32]));
    (845, "UseCardFixExPoint") => UseCardFixExPoint,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(845, "UseCardFixExPoint"), &[EffectType::Usecardfixexpoint as i32]));
    (844, "DeadlyPoison") => DeadlyPoison, stat_read: OnTrigger,
        runtime: |context| buff_act::deadly_poison::runtime_rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [base, compound, cap]
            if *base > 0 && *compound >= 0 && *cap >= 0), wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(844, "DeadlyPoison"), &[EffectType::Deadlypoison as i32]));
    (759, "UseSkillToEnemy") => UseSkillToEnemy, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(759, "UseSkillToEnemy"), &[EffectType::None as i32]));
    (760, "ControlTeamInjuryCountRound") => ControlTeamInjuryCountRound,
        event: EventKind::HpLost, publication: BeforePublish,
        scoped_runtime: |context| buff_act::control_team_injury_count_round::scoped_rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(760, "ControlTeamInjuryCountRound"), &[EffectType::Recordteaminjurycount as i32]));
    (761, "IgnoreDodgeSpecSkill") => IgnoreDodgeSpecSkill, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(761, "IgnoreDodgeSpecSkill"), &[EffectType::Ignoredodgespecskill as i32]));
    (763, "IgnoreRebound") => IgnoreRebound, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(763, "IgnoreRebound"), &[EffectType::Ignorerebound as i32]));
    (764, "CareerRestraint") => CareerRestraint, effect_time_subscription: false,
        non_attack_duration: (buff_act::effect_time::AFTER_HIT_DURATION, AfterHit),
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(764, "CareerRestraint"), &[EffectType::Careerrestraint as i32]));
    (765, "CareerRatioFix") => CareerRatioFix, effect_time_subscription: false,
        supports: buff_act::career_ratio_fix::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(765, "CareerRatioFix"), &[EffectType::None as i32]));
    (745, "AddToBuffEntity") => AddToBuffEntity,
        runtime: |context| buff_act::add_to_buff_entity::rule_ops(context.subscriber, context.event?),
        supports: buff_act::add_to_buff_entity::supports,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(745, "AddToBuffEntity"), &[]));
    (766, "AddToBuffEntity2") => AddToBuffEntity2,
        runtime: |context| buff_act::add_to_buff_entity_2::rule_ops(context.subscriber, context.event?),
        supports: buff_act::add_to_buff_entity_2::supports;
    (767, "AttrFixFromInjuryBank") => AttrFixFromInjuryBank, effect_time_subscription: false,
        stat_read: OnGrant, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(767, "AttrFixFromInjuryBank"), &[EffectType::None as i32]));
    (768, "InjuryLogback") => InjuryLogback, events: [EventKind::HpLost],
        runtime: |context| buff_act::injury_bank::logback_rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [settle_permille] if *settle_permille > 0), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(768, "InjuryLogback"), &[]));
    (769, "AbsorbHurt") => AbsorbHurt, effect_time_subscription: false, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(769, "AbsorbHurt"), &[EffectType::Absorbhurt as i32]));
    (770, "InjuryBank") => InjuryBank, event: EventKind::HpLost,
        transactions: [EventKind::BuffAdded],
        runtime: |context| buff_act::injury_bank::runtime_rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        transaction: buff_act::injury_bank::grant_transaction_rule_ops,
        supports: |args| matches!(args, [raw_attr, cap, skill, threshold, heal, store]
            if crate::engine::entity::attr::AttrId::from_raw(*raw_attr)
                == Some(crate::engine::entity::attr::AttrId::Hp)
                && *cap > 0 && *skill > 0 && *threshold > 0 && *heal > 0 && *store > 0), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(770, "InjuryBank"), &[EffectType::Storageinjury as i32]));
    (771, "MasterHalo") => MasterHalo, references: references_for_feature, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(771, "MasterHalo"), &[EffectType::Masterhalo as i32]));
    (704, "HaloBase") => HaloBase, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(704, "HaloBase"), &[EffectType::Halobase as i32]).with_unchanged_refresh());
    (772, "SlaveHalo") => SlaveHalo, effect_time_subscription: false, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(772, "SlaveHalo"), &[EffectType::Slavehalo as i32]));
    (781, "MockTaunt") => MockTaunt, effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(781, "MockTaunt"), &[EffectType::Mocktaunt as i32]));
    (790, "ModifyAttrByBuffLayer") => ModifyAttrByBuffLayer, effect_time_subscription: false,
        stat_read: OnGrant,
        supports: buff_act::modify_attr_by_buff_layer::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(790, "ModifyAttrByBuffLayer"), &[EffectType::None as i32]));
    (791, "ShieldByBuffLayer") => ShieldByBuffLayer, effect_time_subscription: false,
        supports: buff_act::shield::supports_by_buff_layer, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(791, "ShieldByBuffLayer"), &[EffectType::Shield as i32]));
    (793, "BurnRealHurtFix") => BurnRealHurtFix, effect_time_subscription: false,
        supports: |args| matches!(args, [modifier, floor]
            if (-1000..=1000).contains(modifier) && (0..=1000).contains(floor)),
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(793, "BurnRealHurtFix"), &[EffectType::Realhurtfixwithlimit as i32]));
    (794, "ModifyMaxBurnLayers") => ModifyMaxBurnLayers, effect_time_subscription: false,
        supports: |args| matches!(args, [bonus] if *bonus > 0),
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(794, "ModifyMaxBurnLayers"), &[EffectType::Bufftypenumlimitupdate as i32]));
    (901, "ModifyMaxBuffLayers") => ModifyMaxBuffLayers, effect_time_subscription: false,
        supports: |args| matches!(args, [buff_id, bonus] if *buff_id > 0 && *bonus > 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(901, "ModifyMaxBuffLayers"), &[EffectType::Bufftypenumlimitupdate as i32]));
    (1104, "ButterflyRecordSkill") => ButterflyRecordSkill,
        runtime: |context| buff_act::butterfly_record_skill::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| matches!(args, [count, enchant_id, allowed @ ..]
            if *count > 0 && *enchant_id > 0 && !allowed.is_empty()), initial_state: ButterflyAllowedSkillKinds,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1104, "ButterflyRecordSkill"), &[EffectType::None as i32]));
    (1111, "ToughnessOverflowRecord") => ToughnessOverflowRecord,
        effect_time_subscription: false, transactions: [EventKind::HpLost],
        transaction: buff_act::toughness::transaction_rule_ops,
        supports: |args| args.is_empty(),
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1111, "ToughnessOverflowRecord"), &[EffectType::None as i32]));
    (806, "ExPointOverflowBank") => ExPointOverflowBank,
        events: [EventKind::ExPointChanged, EventKind::PostActionRefill],
        scoped_runtime: |context| buff_act::ex_point_overflow_bank::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(806, "ExPointOverflowBank"), &[EffectType::Expointoverflowbank as i32]));
    (1149, "DeviceExPointOverflowBank") => DeviceExPointOverflowBank,
        events: [EventKind::ExPointChanged, EventKind::PostActionRefill],
        publications: [EventKind::ExPointOverflow => BeforePublish],
        scoped_runtime: |context| buff_act::device_ex_point_overflow_bank::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [capacity] if *capacity > 0),
        initial_state: ZeroInteger,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1149, "DeviceExPointOverflowBank"), &[EffectType::None as i32])
            .without_initial_state_marker());
    (1059, "FictionHp") => FictionHp,
        event: EventKind::RoundStart,
        scoped_runtime: |context| buff_act::fiction_hp::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::fiction_hp::supports,
        hp_loss_state: buff_act::fiction_hp::hp_loss_state_update,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1059, "FictionHp"), &[EffectType::None as i32]));
    (1150, "LostHpToFakeHp") => LostHpToFakeHp, effect_time_subscription: false,
        supports: buff_act::lost_hp_to_fake_hp::supports,
        hp_loss_state: buff_act::lost_hp_to_fake_hp::hp_loss_state_update,
        initial_state: LostHpToFakeHp,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1150, "LostHpToFakeHp"), &[EffectType::None as i32])
            .without_initial_state_marker());
    (1008, "BanLostLife") => BanLostLife,
        supports: |args| matches!(args, [150] | [160]),
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1008, "BanLostLife"), &[EffectType::None as i32]));
    (10001, "AdrenalineAddCard") => AdrenalineAddCard,
        runtime: |context| buff_act::adrenaline_add_card::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::adrenaline_add_card::supports, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(10001, "AdrenalineAddCard"), &[EffectType::None as i32]));
    (10000, "EzioBigSkill") => EzioBigSkill, effect_time_subscription: false, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10000, "EzioBigSkill"), &[EffectType::None as i32]));
    (10002, "AttrByHeroId") => AttrByHeroId, effect_time_subscription: false,
        supports: |args| matches!(args, [raw_attr, _, model_ids @ ..]
            if crate::engine::entity::attr::AttrId::from_raw(*raw_attr).is_some()
                && !model_ids.is_empty()
                && model_ids.iter().all(|model_id| *model_id > 0)), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(10002, "AttrByHeroId"), &[EffectType::Attr as i32]));
    (752, "AttrByDmgType") => AttrByDamageType, effect_time_subscription: false,
        supports: |args| matches!(args, [damage_type @ (1 | 2), raw_attr, _]
            if *damage_type > 0
                && crate::engine::entity::attr::AttrId::from_raw(*raw_attr).is_some()), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::new(DefinitionKey::new(752, "AttrByDmgType"), &[EffectType::Attr as i32], &[], &[]));
    (751, "AttackSkillRateUp") => AttackSkillRateUp, effect_time_subscription: false,
        supports: buff_act::attack_skill_rate_up::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(751, "AttackSkillRateUp"), &[EffectType::None as i32]));
    (10011, "UseBigSkillLike3Rank") => UseBigSkillLike3Rank, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10011, "UseBigSkillLike3Rank"), &[EffectType::None as i32]));
    (10007, "AddAssassinateY") => AddAssassinateY, effect_time_subscription: false,
        supports: buff_act::assassination::supports_source_bonus, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(10007, "AddAssassinateY"), &[EffectType::None as i32]));
    (702, "BuffReplace") => BuffReplace, effect_time_subscription: false, references: references_for_feature, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(702, "BuffReplace"), &[EffectType::Buffreplace as i32]));
    (713, "ExSkillPointChange") => ExSkillPointChange, effect_time_subscription: false,
        transactions: [EventKind::BuffAdded],
        transaction_wire_markers: [EventKind::BuffAdded], frame: CausingFrame,
        transaction: buff_act::ultimate_cost::on_grant,
        supports: |args| matches!(args, [_]), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(713, "ExSkillPointChange"), &[EffectType::Exskillpointchange as i32]));
    (1151, "ImmunityBurn") => ImmunityBurn, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1151, "ImmunityBurn"), &[EffectType::None as i32]));
    (858, "LockBurn") => LockBurn, effect_time_subscription: false,
        supports: |args| matches!(args, [buff_id] if *buff_id > 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(858, "LockBurn"), &[EffectType::Lockburn as i32]));
    (405, "Disarm") => Disarm, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(405, "Disarm"), &[EffectType::Disarm as i32]));
    (406, "Forbid") => Forbid, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(406, "Forbid"), &[EffectType::Forbid as i32]));
    (407, "Seal") => Seal, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(407, "Seal"), &[EffectType::Seal as i32]));
    (10005, "Provoke") => Provoke, effect_time_subscription: false, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10005, "Provoke"), &[EffectType::None as i32]));
    (10003, "AssassinateCreateAdditionalDamage") => AssassinateCreateAdditionalDamage,
        supports: buff_act::assassinate_create_additional_damage::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(10003, "AssassinateCreateAdditionalDamage"), &[EffectType::None as i32]));
    (10004, "BeAttackedAssassinate") => BeAttackedAssassinate,
        event: EventKind::SkillEffectStarted, phase: Immediate, source: Applier, actor: OwnerOrTarget,
        runtime: |context| buff_act::assassination::rule_ops(context.catalog, context.subscriber, context.event?),
        supports: buff_act::assassination::supports_target_trigger,
        parser: buff_act::assassination::parse_target_trigger,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(10004, "BeAttackedAssassinate"), &[EffectType::None as i32]));
    (10006, "BeatBackDependOnAttackMe") => BeatBackDependOnAttackMe,
        event: EventKind::SkillAction, phase: HitPassives, source: Owner, actor: OpposingTeam, timing: AfterAction,
        runtime: |context| buff_act::riposte::rule_ops(context.pool, context.subscriber, context.event?),
        supports: buff_act::riposte::supports_dependent, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10006, "BeatBackDependOnAttackMe"), &[EffectType::None as i32]));
    (815, "AddSpTempCard") => AddSpTempCard,
        scoped_runtime: |context| {
            let reserve_id = i64::from(context.pool.entity(context.subscriber.owner_uid)?.model_id);
            buff_act::add_sp_temp_card::subscriber_rule_ops(context.managers, context.subscriber, context.event?, reserve_id)
        },
        supports: |_| true, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(815, "AddSpTempCard"), &[EffectType::None as i32]));
    (820, "AttrFromEntity") => AttrFromEntity, effect_time_subscription: false, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(820, "AttrFromEntity"), &[EffectType::Attr as i32]));
    (1152, "AttrFromEntityNoLayer") => AttrFromEntityNoLayer, effect_time_subscription: false,
        supports: buff_act::attr_from_entity_no_layer::supports, state_consumer: true,
        initial_state: SourceAttackAttribute,
        wire: (buff_act::wire::BuffActWireDefinition::new(
            DefinitionKey::new(1152, "AttrFromEntityNoLayer"),
            &[EffectType::Attr as i32],
            &[EffectType::Attr as i32],
            &[],
        ).with_initial_state_refresh_projection());
    (822, "LayerMasterHalo") => LayerMasterHalo, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(822, "LayerMasterHalo"), &[EffectType::Layermasterhalo as i32]));
    (825, "ConsumeBuffContinueChannel") => ConsumeBuffContinueChannel, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(825, "ConsumeBuffContinueChannel"), &[EffectType::None as i32]));
    (826, "LockBulletCountDecr") => LockBulletCountDecr,
        effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(826, "LockBulletCountDecr"), &[EffectType::Lockbulletcountdecr as i32]));
    (827, "Bullet") => Bullet,
        events: [EventKind::BuffFeatureTriggered],
        source: Applier,
        timing: AfterSkill,
        timings: [EventKind::BuffFeatureTriggered => AfterHitObservers],
        scoped_runtime: buff_act::bullet::reaction,
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(827, "Bullet"), &[EffectType::None as i32]));
}
