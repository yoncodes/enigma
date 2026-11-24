use sonettobuf::effect_type_enum::EffectType;

use super::super::*;
use crate::engine::skill::buff_act;

buff_act_definitions! {
    (100, "Attr") => Attr,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        transaction_wire_markers: [EventKind::BuffChanged],
        publications: [
            EventKind::BuffAdded => BeforePublish
        ],
        frame: CausingFrame,
        transaction: buff_act::attr::transaction_rule_ops,
        replacement_publication: BeforePublish,
        wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(100, "Attr"), &[EffectType::Attr as i32]).with_attr_max_hp());
    (108, "Attr") => UltimateAttribute,
        effect_time_subscription: false,
        supports: buff_act::ultimate_attribute::supports,
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(108, "Attr"), &[EffectType::Attr as i32]));
    (111, "Attr") => AttrOnlyCalDamageBasicAttacked,
        effect_time_subscription: false,
        supports: buff_act::attr_only_cal_damage_attack::supports_basic_attacked,
        state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(111, "Attr"), &[EffectType::Attr as i32]));
    (10021, "Rouge2AttrToRole") => Rouge2AttrToRole,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        transaction: buff_act::rouge2_attr_to_role::transaction_rule_ops,
        supports: buff_act::rouge2_attr_to_role::supports,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(10021, "Rouge2AttrToRole"), &[EffectType::None as i32]).with_max_hp(1, 0));
    (853, "AttrByLostHp") => AttrByLostHp, effect_time_subscription: false,
        supports: buff_act::attr_by_lost_hp::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(853, "AttrByLostHp"), &[EffectType::None as i32]));
    (1056, "AttrByLostHp") => AttrByLostHp, effect_time_subscription: false,
        supports: buff_act::attr_by_lost_hp::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(1056, "AttrByLostHp"), &[EffectType::None as i32]));
    (201, "Cure") => Cure,
        runtime: |context| buff_act::cure::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| buff_act::cure::supports(BuffActKind::Cure, args), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(201, "Cure"), &[EffectType::Cure as i32]));
    (202, "Dot") => Dot, stat_read: ByArguments,
        runtime: |context| Some(buff_act::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: buff_act::damage_over_time::supports_round_end_dot, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(202, "Dot"), &[EffectType::Dot as i32]));
    (203, "Dot") => Dot, stat_read: ByArguments,
        runtime: |context| Some(buff_act::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: buff_act::damage_over_time::supports_dot, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(203, "Dot"), &[EffectType::Dot as i32]));
    (213, "Dot") => Dot, stat_read: ByArguments,
        runtime: |context| Some(buff_act::damage_over_time::layered_damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: buff_act::damage_over_time::supports_dot, wire: (buff_act::wire::BuffActWireDefinition::new(DefinitionKey::new(213, "Dot"), &[], &[EffectType::Dot as i32], &[]));
    (512, "Cure") => Cure,
        runtime: |context| buff_act::revive::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::revive::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(512, "Cure"), &[EffectType::Cure as i32]));
    (849, "AdvancedCure") => AdvancedCure, events: [EventKind::BeAttacked],
        owns_duration: true,
        runtime_marker: BeforeChanges(Owner),
        runtime: |context| buff_act::cure::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| buff_act::cure::supports(BuffActKind::AdvancedCure, args), wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(849, "AdvancedCure"), &[EffectType::None as i32]));
    (113, "AttrOnlyCalDamageAttack") => AttrOnlyCalDamageAttack,
        supports: |args| matches!(args, [_, _, consume, ..] if *consume != 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(113, "AttrOnlyCalDamageAttack"), &[EffectType::None as i32]));
    (1001, "AttrOnlyCalDamageAttackBigSkill") => AttrOnlyCalDamageAttackBigSkill,
        effect_time_subscription: false,
        supports: |args| matches!(args, [_, _, consume, ..] if *consume != 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(1001, "AttrOnlyCalDamageAttackBigSkill"), &[EffectType::Attr as i32]));
    (112, "AttrOnlyCalDamageBeAttacked") => AttrOnlyCalDamageBeAttacked,
        effect_time_subscription: false,
        supports: buff_act::attr_only_cal_damage_attack::supports_be_attacked, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(112, "AttrOnlyCalDamageBeAttacked"), &[EffectType::None as i32]));
    (114, "AttrOnlyCalDamageBeAttackedType") => AttrOnlyCalDamageBeAttackedType,
        effect_time_subscription: false,
        supports: buff_act::attr_only_cal_damage_attack::supports_be_attacked_type, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(114, "AttrOnlyCalDamageBeAttackedType"), &[EffectType::None as i32]));
    (740, "AttrOnlyCalDamageInExtra") => AttrOnlyCalDamageInExtra,
        effect_time_subscription: false,
        supports: buff_act::attr_only_cal_damage_attack::supports_extra_action, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(740, "AttrOnlyCalDamageInExtra"), &[EffectType::None as i32]));
    (106, "AttrSkillSingle") => AttrSkillSingle, effect_time_subscription: false,
        supports: buff_act::attr_by_skill_target_count::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(106, "AttrSkillSingle"), &[EffectType::None as i32]));
    (107, "AttrSkillMultiple") => AttrSkillMultiple, effect_time_subscription: false,
        supports: buff_act::attr_by_skill_target_count::supports, state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(107, "AttrSkillMultiple"), &[EffectType::None as i32]));
    (739, "CreateHeroTempCards") => CreateHeroTempCards,
        scoped_runtime: |context| buff_act::add_sp_temp_card::hero_skill_subscriber_rule_ops(context.pool, context.subscriber, context.event?),
        supports: buff_act::add_sp_temp_card::supports_hero_skill,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(739, "CreateHeroTempCards"), &[EffectType::None as i32]));
    (10015, "CreateTempSkill3Card") => CreateTempSkill3Card,
        scoped_runtime: |context| buff_act::add_sp_temp_card::configured_skill3_subscriber_rule_ops(
            context.subscriber,
            context.event?,
            context.pool.entity(context.subscriber.owner_uid)?.model_id,
        ),
        supports: buff_act::add_sp_temp_card::supports_configured_skill3,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(10015, "CreateTempSkill3Card"), &[EffectType::None as i32]));
    (302, "BeatBack") => BeatBack,
        event: EventKind::SkillAction, phase: HitPassives, source: Owner, actor: OpposingTeam, timing: AfterAction,
        runtime: |context| buff_act::riposte::holder_rule_ops(context.pool, context.subscriber, context.event?),
        supports: buff_act::riposte::supports_holder, references: references_for_feature, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(302, "BeatBack"), &[EffectType::Beatback as i32]));
    (301, "Taunt") => Taunt, effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(301, "Taunt"), &[EffectType::Taunt as i32]));
    (303, "Rebound") => Rebound, source: Owner,
        timing_override: buff_act::rebound::timing,
        multiplicity: OncePerActionTarget,
        runtime: |context| buff_act::rebound::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::rebound::supports, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(303, "Rebound"), &[EffectType::Rebound as i32]));
    (305, "AddToAttacker") => AddToAttacker,
        runtime_marker: BeforeChanges(EventSource),
        scoped_runtime: |context| buff_act::add_to_target::scoped_rule_ops(context.managers, context.subscriber, context.event?, context.catalog, context.pool),
        supports: |args| matches!(args, [buff_id] if *buff_id > 0),
        wire: (buff_act::wire::BuffActWireDefinition::new(DefinitionKey::new(305, "AddToAttacker"), &[], &[EffectType::Addtoattacker as i32], &[]));
    (306, "ExPointFix") => ExPointFix, event: EventKind::BeAttacked,
        runtime: |context| buff_act::ex_point_fix::rule_ops(context.subscriber, context.event?),
        supports: buff_act::ex_point_fix::supports_holder,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(306, "ExPointFix"), &[])
            .without_initial_state_marker());
    (307, "ExPointFix") => ExPointFix, event: EventKind::BeAttacked,
        runtime: |context| buff_act::ex_point_fix::rule_ops(context.subscriber, context.event?),
        supports: buff_act::ex_point_fix::supports_attacker,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(307, "ExPointFix"), &[])
            .without_initial_state_marker());
    (401, "Dizzy") => Dizzy, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(401, "Dizzy"), &[EffectType::Dizzy as i32]));
    (706, "PassiveSkillInvalid") => PassiveSkillInvalid, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(706, "PassiveSkillInvalid"), &[EffectType::Passiveskillinvalid as i32]));
    (402, "Petrified") => Petrified, event: EventKind::TargetAttacked, frame: CausingFrame,
        runtime: |context| buff_act::petrified::rule_ops(context.catalog, context.subscriber, context.event?),
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(402, "Petrified"), &[]));
    (403, "Sleep") => Sleep, event: EventKind::TargetAttacked, frame: CausingFrame,
        runtime: |context| buff_act::sleep::rule_ops(context.subscriber, context.event?),
        supports: |args| args.is_empty(), wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(403, "Sleep"), &[EffectType::Sleep as i32]));
    (404, "Frozen") => Frozen, event: EventKind::TargetAttacked, frame: CausingFrame,
        runtime: |context| buff_act::frozen::rule_ops(context.catalog, context.subscriber, context.event?),
        supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(404, "Frozen"), &[EffectType::Frozen as i32]));
    (501, "Shield") => Shield, effect_time_subscription: false,
        supports: buff_act::shield::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(501, "Shield"), &[EffectType::Shield as i32]));
    (511, "FixedHurt") => FixedHurt, effect_time_subscription: false,
        supports: buff_act::fixed_hurt::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(511, "FixedHurt"), &[EffectType::Fixedhurt as i32]));
    (503, "AddToTarget") => AddToTarget,
        publications: [
            EventKind::SkillCast => BeforePublish
        ],
        scoped_runtime: |context| buff_act::add_to_target::scoped_rule_ops(context.managers, context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(503, "AddToTarget"), &[EffectType::Addtotarget as i32]));
    (505, "DodgeSpecSkill") => DodgeSpecSkill, effect_time_subscription: false,
        events: [EventKind::AllyAction],
        scoped_runtime: |context| buff_act::dodge_spec_skill::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::dodge_spec_skill::supports_skill_slots, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(505, "DodgeSpecSkill"), &[EffectType::Dodgespecskill as i32]));
    (507, "DodgeSpecSkill2") => DodgeDamageType, effect_time_subscription: false,
        events: [EventKind::AllyAction, EventKind::BuffFeatureTriggered],
        scoped_runtime: |context| buff_act::dodge_spec_skill::rule_ops(context.managers, context.subscriber, context.event?),
        supports: buff_act::dodge_spec_skill::supports_damage_types, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(507, "DodgeSpecSkill2"), &[EffectType::Dodgespecskill2 as i32]));
    (510, "DamageNotMoreThan") => DamageNotMoreThan, effect_time_subscription: false,
        events: [EventKind::TargetAttacked],
        runtime: |context| buff_act::damage_not_more_than::consume_after_hit(context.managers, context.subscriber, context.event?),
        supports: buff_act::damage_not_more_than::supports, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add(DefinitionKey::new(510, "DamageNotMoreThan"), &[EffectType::Damagenotmorethan as i32]));
    (509, "ImmunityExpointChange") => MoxieReductionImmunity,
        effect_time_subscription: false, supports: |args| args.is_empty(), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(509, "ImmunityExpointChange"), &[EffectType::None as i32]));
    (518, "AddToTarget") => AddToTarget,
        scoped_runtime: |context| buff_act::add_to_target::scoped_rule_ops(context.managers, context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true;
    (519, "RealHurtFix") => RealHurtFix, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(519, "RealHurtFix"), &[EffectType::Realhurtfix as i32]));
    (520, "RealHarmFix") => RealHarmFix, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(520, "RealHarmFix"), &[EffectType::Realharmfix as i32]));
    (522, "RealHarmSkillEffectFix") => RealHarmSkillEffectFix, effect_time_subscription: false, state_consumer: true, wire: (buff_act::wire::BuffActWireDefinition::all(DefinitionKey::new(522, "RealHarmSkillEffectFix"), &[EffectType::Realharmskilleffectfix as i32]));
    (601, "Injury") => Injury, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true,
        wire: (buff_act::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(601, "Injury"), &[EffectType::Injury as i32]));
}
