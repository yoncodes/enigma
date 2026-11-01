use super::*;
use crate::engine::entity::attr::AttrId;
use crate::engine::manager::gauge::{GaugeCommand, GaugeOperation};
use crate::engine::skill::behavior::{magic_circle, rate};

const EMITTER: i64 = crate::engine::manager::emitter::UID;
const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "ImpromptuDamageRegression"),
};

fn fixture(inspiration: i32) -> (Fight, TargetPool, BattleManagers, SkillEffectCatalog) {
    init_config();
    let mut emitter = crate::engine::manager::emitter::activation_entity();
    emitter.buffs = [31080151, 31411402, 31410410].into_iter().enumerate()
        .map(|(i, buff_id)| BuffInfo { uid: Some(30 + i as i64), buff_id: Some(buff_id),
            from_uid: Some(10), ..Default::default() }).collect();
    let owner = FightEntityInfo {
        uid: Some(10), model_id: Some(3141), position: Some(1), current_hp: Some(10_000),
        team_type: Some(1), attr: Some(HeroAttribute { hp: Some(10_000), attack: Some(2792),
            ..Default::default() }),
        buffs: vec![BuffInfo { uid: Some(20), buff_id: Some(2240000), from_uid: Some(10),
            ..Default::default() }], ..Default::default()
    };
    let target = FightEntityInfo {
        uid: Some(-1), position: Some(1), team_type: Some(2), current_hp: Some(10_000_000),
        weak_careers: vec![3],
        attr: Some(HeroAttribute { hp: Some(10_000_000), defense: Some(1200),
            mdefense: Some(1000), ..Default::default() }), ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![owner], emitter: Some(emitter), ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![target], ..Default::default() }), ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let enabled = enable_rule_ops(crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge, &managers.buff.active_features(&managers.hp), EMITTER).pop().unwrap();
    if let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter {
        managers.execute_emitter(command);
    }
    for op in [enabled.team_energy, enabled.inspiration] {
        if let RuleOp::Command(BattleCommand::Gauge(command)) = op {
            managers.execute_gauge(command).unwrap();
        }
    }
    // Production enables the virtual emitter first, then grants its effects.
    // FightTeam.emitter is a protocol snapshot, not a seeded combatant.
    for buff_id in [31080151, 31411402, 31410410] {
        managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
            crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: 10,
                target_uid: EMITTER, buff_id, amount: None, params: None, act_info: None })).unwrap();
    }
    managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
        GaugeOperation::ChangeValue { delta: inspiration })).unwrap();
    let pool = TargetPool::from_fight(&fight).runtime_view(&managers);
    let effects = SkillEffectCatalog::from_roots(crate::test_support::game_data(),
        [2240001, 31411001, 31414153, 31413151], [31080151, 31411402, 31410410]);
    (fight, pool, managers, effects)
}

fn deploy(managers: &mut BattleManagers, id: i32) {
    managers.field.execute_command(FieldCommand { origin: ORIGIN, team: 1,
        operation: FieldOperation::DeployIfAbsent {
            definition: FieldDefinition { field_id: id, duration: 4 }, create_uid: 10,
            initial_level: 3, thresholds: vec![],
        }, }).unwrap();
}

// Use a real hand card and the same virtual emitter setup as an actual fight.
fn eureka_card_fixture(
    hero_id: i32,
    skill_id: i32,
    power: i32,
    quartet: bool,
) -> (Fight, TargetPool, BattleManagers, SkillEffectCatalog, i64) {
    let (mut fight, _, _, _) = fixture(1);
    let caster = if hero_id == 3141 { 10 } else { 11 };
    if caster == 11 {
        fight.attacker.as_mut().unwrap().entitys.push(FightEntityInfo {
            uid: Some(caster), model_id: Some(hero_id), position: Some(2),
            team_type: Some(1), current_hp: Some(10_000),
            attr: Some(HeroAttribute { hp: Some(10_000), attack: Some(2792), ..Default::default() }),
            ..Default::default()
        });
    }
    if quartet {
        let allies = &mut fight.attacker.as_mut().unwrap().entitys;
        allies.push(FightEntityInfo {
            uid: Some(12), model_id: Some(3001), position: Some(3), team_type: Some(1),
            current_hp: Some(10_000),
            attr: Some(HeroAttribute { hp: Some(10_000), attack: Some(2792), ..Default::default() }),
            ..Default::default()
        });
        for (index, ally) in allies.iter_mut().enumerate() {
            ally.buffs.push(BuffInfo { uid: Some(100 + index as i64), buff_id: Some(31080131),
                from_uid: Some(caster), duration: Some(5), ..Default::default() });
        }
    }
    let mut managers = BattleManagers::seeded(&fight);
    let enabled = enable_rule_ops(crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge, &managers.buff.active_features(&managers.hp), EMITTER).pop().unwrap();
    if let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter {
        managers.execute_emitter(command);
    }
    for op in [enabled.team_energy, enabled.inspiration] {
        if let RuleOp::Command(BattleCommand::Gauge(command)) = op {
            managers.execute_gauge(command).unwrap();
        }
    }
    for buff_id in [31080151, 31411402, 31410410] {
        managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
            crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: 10,
                target_uid: EMITTER, buff_id, amount: None, params: None, act_info: None })).unwrap();
    }
    managers.eureka.resync_observed(caster, 1, power, 10).unwrap();
    managers.execute_card(CardCommand::Setup(CardSetup {
        hand: vec![CardInfo { uid: Some(caster), skill_id: Some(skill_id), energy: Some(0),
            ..Default::default() }], draw_pile: Vec::new(), deck_num: 1,
    })).unwrap();
    let pool = TargetPool::from_fight(&fight).runtime_view(&managers);
    let effects = SkillEffectCatalog::from_roots(crate::test_support::game_data(),
        [skill_id, 2240001, 31411001], [2240000, 31080151, 31411402, 31410410]);
    (fight, pool, managers, effects, caster)
}

fn play_eureka_card(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    effects: &SkillEffectCatalog,
    target_uid: i64,
) -> DrainResult {
    run_player_action_queue(managers, pool, effects, &mut RoundDeterminism::default(),
        TargetContext::default(), [CardPlay { origin: CARD_PLAY_ORIGIN, hand_index: 0,
            target_uid: Some(target_uid), chosen_skill_id: None, choice: None,
            recorded_skill: None }], 1, EMITTER).unwrap()
}

#[test]
fn eureka_card_extra_hit_is_applied_to_the_actual_impromptu() {
    for (hero, skill, buff) in [
        (3141, 31410111, 31410101), (3141, 31410112, 31410101),
        (3141, 31410113, 31410101), (3117, 31170111, 31170011),
        (3117, 31170112, 31170011), (3117, 31170113, 31170011),
        (3053, 30530161, 30530302), (3048, 30480211, 30480212),
        (3113, 31130111, 31130112),
    ] {
        let (_, pool, mut managers, effects, caster) = eureka_card_fixture(hero, skill, 2, false);
        play_eureka_card(&mut managers, &pool, &effects, -1);
        assert_eq!(managers.eureka.get(caster, 1).current, 0, "skill {skill}");
        assert_eq!(managers.buff.buff_id_amount(EMITTER, buff), 1, "skill {skill}");
        let plan = build_plan(&managers, 1, EMITTER).unwrap();
        assert_eq!((plan.inspiration, plan.attack_count), (2, 2), "skill {skill}");
        let before = managers.hp.current(-1);
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(31411001, EMITTER, [true; 8]);
        let result = run_impromptu(&mut managers, &pool, &effects, &mut determinism,
            TargetContext::default(), 1, EMITTER).unwrap();
        assert!(result.events.iter().any(|event| matches!(event,
            BattleEvent::ImpromptuResolved { critical_action_count: 2, .. })), "skill {skill}");
        assert!(managers.hp.current(-1) < before, "skill {skill}");
    }
}

#[test]
fn eureka_card_bonus_changes_actual_impromptu_damage_only_after_payment() {
    fn damage(hero: i32, skill: i32, initial_power: i32, expected_bonus: i32) -> i32 {
        let (_, pool, mut managers, effects, caster) = eureka_card_fixture(hero, skill, initial_power, false);
        play_eureka_card(&mut managers, &pool, &effects, caster);
        assert_eq!(managers.eureka.get(caster, 1).current, if initial_power >= 2 { 0 } else { 1 });
        assert_eq!(managers.buff.attribute_delta(EMITTER, AttrId::DmgBonus), expected_bonus);
        // Compare the same Inspiration: the zero-payment case naturally did not gain it.
        if initial_power < 2 {
            managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
                GaugeOperation::ChangeValue { delta: 2 })).unwrap();
        }
        assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().inspiration, 2);
        let before = managers.hp.current(-1);
        run_impromptu(&mut managers, &pool, &effects, &mut RoundDeterminism::default(),
            TargetContext::default(), 1, EMITTER).unwrap();
        before - managers.hp.current(-1)
    }
    for (hero, skill, bonus) in [(3141, 31410121, 200), (3141, 31410122, 250),
        (3141, 31410123, 300), (3053, 30530171, 200)] {
        let baseline = damage(hero, skill, 1, 0);
        let boosted = damage(hero, skill, 2, bonus);
        assert!(baseline > 0 && boosted > baseline, "skill {skill}: {baseline} -> {boosted}");
    }
}

#[test]
fn eureka_card_bonuses_accumulate_for_repeated_cards_in_one_round() {
    for (hero, skill, buff, expected_count, expected_bonus) in [
        (3141, 31410111, 31410101, 4, 0),
        (3117, 31170111, 31170011, 4, 0),
        (3141, 31410121, 31410203, 2, 400),
    ] {
        let (_, pool, mut managers, effects, caster) = eureka_card_fixture(hero, skill, 4, false);
        managers.execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo { uid: Some(caster), skill_id: Some(skill), energy: Some(0),
                ..Default::default() }; 2], draw_pile: Vec::new(), deck_num: 2,
        })).unwrap();
        let target = if skill == 31410121 { caster } else { -1 };
        let played = run_player_action_queue(&mut managers, &pool, &effects, &mut RoundDeterminism::default(),
            TargetContext::default(), (0..2).map(|_| CardPlay {
                origin: CARD_PLAY_ORIGIN, hand_index: 0, target_uid: Some(target),
                chosen_skill_id: None, choice: None, recorded_skill: None,
            }), 1, EMITTER).unwrap();
        assert_eq!(managers.eureka.get(caster, 1).current, 0, "skill {skill}");
        assert_eq!(managers.buff.buff_id_amount(EMITTER, buff), 2, "skill {skill}");
        let buff_uids = managers.buff.active_features_for_owner(&managers.hp, EMITTER)
            .into_iter().filter(|feature| feature.buff_id == buff)
            .map(|feature| feature.buff_uid).collect::<std::collections::HashSet<_>>();
        assert_eq!(buff_uids.len(), 2, "skill {skill}: each bonus needs a separate buff UID");
        fn added_bonus_uids(step: &sonettobuf::FightStep, buff_id: i32, out: &mut Vec<i64>) {
            for effect in &step.act_effect {
                if effect.target_id == Some(EMITTER)
                    && effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Buffadd as i32)
                    && let Some(buff) = effect.buff.as_ref()
                    && buff.buff_id == Some(buff_id)
                    && let Some(uid) = buff.uid
                {
                    out.push(uid);
                }
                if let Some(child) = effect.fight_step.as_ref() {
                    added_bonus_uids(child, buff_id, out);
                }
            }
        }
        let mut sent_uids = Vec::new();
        for step in crate::engine::packet::timeline::project(&played.frames).unwrap() {
            added_bonus_uids(&step, buff, &mut sent_uids);
        }
        assert_eq!(sent_uids.into_iter().collect::<std::collections::HashSet<_>>(), buff_uids,
            "skill {skill}: both bonuses must be sent to the client");
        assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().attack_count, expected_count, "skill {skill}");
        assert_eq!(managers.buff.attribute_delta(EMITTER, AttrId::DmgBonus), expected_bonus, "skill {skill}");
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(31411001, EMITTER, [true; 8]);
        let result = run_impromptu(&mut managers, &pool, &effects, &mut determinism,
            TargetContext::default(), 1, EMITTER).unwrap();
        assert!(result.events.iter().any(|event| matches!(event,
            BattleEvent::ImpromptuResolved { critical_action_count, .. }
                if *critical_action_count == expected_count)), "skill {skill}");
        run_attacker_round_end(&mut managers, &pool, &effects, &mut RoundDeterminism::default(),
            TargetContext::default()).unwrap();
        assert_eq!(managers.buff.buff_id_amount(EMITTER, buff), 0, "skill {skill}: bonus must expire");
    }
}

#[test]
fn eureka_card_barcarola_counts_base_and_quartet_extra_attack_bonuses() {
    for (skill, base, with_quartet) in [
        (31080111, 1, 2), (31080114, 2, 3), (31080117, 2, 4),
    ] {
        for (quartet, extra) in [(false, base), (true, with_quartet)] {
            let (_, pool, mut managers, effects, caster) = eureka_card_fixture(3108, skill, 2, quartet);
            managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
                GaugeOperation::ChangeValue { delta: 2 })).unwrap();
            play_eureka_card(&mut managers, &pool, &effects, -1);
            assert_eq!(managers.eureka.get(caster, 1).current, 0, "skill {skill}, quartet {quartet}");
            assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().attack_count, 1 + extra,
                "skill {skill}, quartet {quartet}");
        }
    }
}

#[test]
fn eureka_card_barcarola_counts_base_and_quartet_extra_damage_bonuses() {
    fn damage(skill: i32, quartet: bool, power: i32, expected_bonus: i32) -> i32 {
        let (_, pool, mut managers, effects, caster) = eureka_card_fixture(3108, skill, power, quartet);
        managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
            GaugeOperation::ChangeValue { delta: 2 })).unwrap();
        play_eureka_card(&mut managers, &pool, &effects, -1);
        // Quartet can spend one additional point from its own shared effect.
        if power >= 2 || !quartet {
            assert_eq!(managers.eureka.get(caster, 1).current, if power >= 2 { 0 } else { 1 });
        }
        assert_eq!(managers.buff.attribute_delta(EMITTER, AttrId::DmgBonus), expected_bonus,
            "skill {skill}, quartet {quartet}");
        let before = managers.hp.current(-1);
        run_impromptu(&mut managers, &pool, &effects, &mut RoundDeterminism::default(),
            TargetContext::default(), 1, EMITTER).unwrap();
        before - managers.hp.current(-1)
    }
    for (skill, base, with_quartet) in [(31080121, 200, 400), (31080124, 350, 550)] {
        for (quartet, bonus) in [(false, base), (true, with_quartet)] {
            let baseline = damage(skill, quartet, 1, 0);
            let boosted = damage(skill, quartet, 2, bonus);
            assert!(baseline > 0 && boosted > baseline,
                "skill {skill}, quartet {quartet}: {baseline} -> {boosted}");
        }
    }
}

#[test]
fn eureka_card_extra_hit_over_the_circle_cap_increases_actual_damage() {
    fn run(paid_card: bool) -> (i32, i32, i32) {
        let (_, pool, mut managers, effects, _) = eureka_card_fixture(3141, 31410111, 2, false);
        let inspiration = if paid_card { 18 } else { 20 };
        managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
            GaugeOperation::ChangeValue { delta: inspiration })).unwrap();
        deploy(&mut managers, 45004);
        if paid_card {
            play_eureka_card(&mut managers, &pool, &effects, -1);
        }
        let plan = build_plan(&managers, 1, EMITTER).unwrap();
        assert_eq!(plan.inspiration, 20);
        let before = managers.hp.current(-1);
        let mut determinism = RoundDeterminism::default();
        determinism.enqueue_hidden_crits(31411001, EMITTER, [true; 8]);
        let result = run_impromptu(&mut managers, &pool, &effects, &mut determinism,
            TargetContext::default(), 1, EMITTER).unwrap();
        assert!(result.events.iter().any(|event| matches!(event,
            BattleEvent::ImpromptuResolved { critical_action_count: 5, .. })));
        (plan.attack_count, plan.damage_rate, before - managers.hp.current(-1))
    }
    let base = run(false);
    let extra = run(true);
    assert_eq!((base.0, extra.0), (5, 5));
    assert_eq!(extra.1 - base.1, 3000);
    assert!(extra.2 > base.2, "base={base:?}, extra={extra:?}");
}

#[test]
fn eureka_card_no_attack_bonus_without_the_required_power() {
    for (hero, skill, buff) in [
        (3141, 31410111, 31410101), (3117, 31170111, 31170011),
        (3053, 30530161, 30530302), (3108, 31080111, 31080111),
    ] {
        let (_, pool, mut managers, effects, caster) = eureka_card_fixture(hero, skill, 1, false);
        managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
            GaugeOperation::ChangeValue { delta: 2 })).unwrap();
        play_eureka_card(&mut managers, &pool, &effects, -1);
        assert_eq!(managers.eureka.get(caster, 1).current, 1, "skill {skill}");
        assert_eq!(managers.buff.buff_id_amount(EMITTER, buff), 0, "skill {skill}");
        assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().attack_count, 1, "skill {skill}");
    }
}

#[test]
fn inspiration_attack_thresholds_are_cumulative_and_do_not_double_count_child_buffs() {
    for (energy, expected) in [(1, 1), (2, 1), (3, 2), (5, 2), (6, 3), (11, 3),
        (12, 4), (19, 4), (20, 6), (24, 6), (27, 6)] {
        let (_, _, mut managers, _) = fixture(energy);
        assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().attack_count, expected,
            "inspiration={energy}");
        // An internal threshold grant must not add the same attacks twice.
        managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
            crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: EMITTER,
                target_uid: EMITTER, buff_id: 31080155, amount: Some(0), params: None,
                act_info: None })).unwrap();
        assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().attack_count, expected);
    }
}

#[test]
fn maximum_circle_adds_emitter_rates_attributes_and_one_overflow_at_24_inspiration() {
    let (_, pool, mut managers, effects) = fixture(24);
    deploy(&mut managers, 45004);
    let catalog = managers.catalog();
    assert_eq!(managers.field.attribute_delta(catalog, EMITTER, AttrId::DmgBonus, &pool), 250);
    assert_eq!(managers.field.attribute_delta(catalog, EMITTER, AttrId::Penetration, &pool), 200);
    assert_eq!(managers.field.attribute_delta(catalog, 10, AttrId::DmgBonus, &pool), 0);
    assert_eq!(managers.field.attribute_delta(catalog, -1, AttrId::Penetration, &pool), 0);
    assert_eq!(magic_circle::active_self_skills(EMITTER, &managers, &pool), vec![31414153]);
    assert!(magic_circle::active_self_skills(10, &managers, &pool).is_empty());
    let plan = build_plan(&managers, 1, EMITTER).unwrap();
    assert_eq!((plan.attack_count, plan.damage_rate), (5, 6600));
    assert_eq!(pool.entity(EMITTER).unwrap().damage_type,
        crate::engine::skill::target::EntityDamageType::Reality);
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    rate::emit_passive_attack_attributes(&mut modifiers, EMITTER, plan.skill_id, &[],
        rate::RateRuntime { effects: &effects, managers: &managers, pool: &pool,
            context: TargetContext { active_skill_id: plan.skill_id,
                active_skill_source_uid: EMITTER, ..Default::default() } },
        &mut RoundDeterminism::default());
    assert_eq!(modifiers.rates.iter().filter_map(|rate| rate.fixed_value()).sum::<i32>(), 11000);
    assert_eq!(crate::engine::skill::buff_act::emitter_num_change::split_final_damage_delta_with_circle(
        &managers, 1, EMITTER, 2), 0);
    managers.field.execute_command(FieldCommand { origin: ORIGIN, team: 1,
        operation: FieldOperation::Remove }).unwrap();
    assert_eq!(build_plan(&managers, 1, EMITTER).unwrap().attack_count, 6);
    assert_eq!(managers.field.attribute_delta(catalog, EMITTER, AttrId::DmgBonus, &pool), 0);
    assert!(magic_circle::active_self_skills(EMITTER, &managers, &pool).is_empty());
    assert_eq!(crate::engine::skill::buff_act::emitter_num_change::split_final_damage_delta_with_circle(
        &managers, 1, EMITTER, 2), -700);
}

#[test]
fn impromptu_executes_five_hits_with_the_maximum_circle() {
    let (_, pool, mut managers, effects) = fixture(24);
    deploy(&mut managers, 45004);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_hidden_crits(31411001, EMITTER, [true; 6]);
    let result = run_impromptu(&mut managers, &pool, &effects, &mut determinism,
        TargetContext::default(), 1, EMITTER).unwrap();
    assert!(result.events.iter().any(|event| matches!(event,
        BattleEvent::ImpromptuResolved { critical_action_count: 5, .. })));
    assert_eq!(managers.gauge.get(inspiration_key(EMITTER)).unwrap().current, 0);
    assert!(managers.hp.current(-1) < 10_000_000);
}

fn apply_field_ops(managers: &mut BattleManagers, ops: Vec<RuleOp>) {
    for op in ops {
        let RuleOp::Command(BattleCommand::Field(command)) = op else {
            panic!("expected an authoritative field command");
        };
        managers.execute_field(command).unwrap();
    }
}

#[test]
fn emitter_circle_upgrades_refreshes_expires_to_level_two_and_remembers_redeployment() {
    let (_, pool, mut managers, _) = fixture(24);
    let add = ParsedBehavior::new(50019, "AddMagicCircle", vec![45001]);
    let ops = magic_circle::deploy_rule_ops(&add, 10, 1, &managers, &pool).unwrap();
    apply_field_ops(&mut managers, ops);
    assert_eq!(managers.field.get(1).unwrap().definition.duration, -1);
    assert!(magic_circle::emitter_circle_round_end_rule_ops(&managers, 1).is_empty());
    let upgrade = ParsedBehavior::new(60270, "UpdateWangQiMagicCircle", vec![1]);
    for id in [45002, 45003, 45004] {
        let ops = magic_circle::upgrade_emitter_circle_rule_ops(&upgrade, 1, &managers).unwrap();
        apply_field_ops(&mut managers, ops);
        assert_eq!(managers.field.get(1).unwrap().definition.field_id, id);
    }
    assert_eq!(managers.field.get(1).unwrap().definition.duration, 4);
    for remaining in [3, 2, 1] {
        let ops = magic_circle::emitter_circle_round_end_rule_ops(&managers, 1);
        apply_field_ops(&mut managers, ops);
        assert_eq!(managers.field.get(1).unwrap().definition.duration, remaining);
    }
    let ops = magic_circle::upgrade_emitter_circle_rule_ops(&upgrade, 1, &managers).unwrap();
    apply_field_ops(&mut managers, ops);
    assert_eq!(managers.field.get(1).unwrap().definition.duration, 4);
    for _ in 0..4 {
        let ops = magic_circle::emitter_circle_round_end_rule_ops(&managers, 1);
        apply_field_ops(&mut managers, ops);
    }
    assert_eq!(managers.field.get(1).unwrap().definition.field_id, 45003);
    assert_eq!(managers.field.get(1).unwrap().definition.duration, -1);
    managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
        crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: 10,
            target_uid: 10, buff_id: 31410409, amount: None, params: None, act_info: None })).unwrap();
    managers.execute_field(FieldCommand { origin: ORIGIN, team: 1,
        operation: FieldOperation::Remove }).unwrap();
    let ops = magic_circle::deploy_rule_ops(&add, 10, 1, &managers, &pool).unwrap();
    apply_field_ops(&mut managers, ops);
    assert_eq!(managers.field.get(1).unwrap().definition.field_id, 45003);
}

#[test]
fn emitter_circle_insight_might_uses_the_same_level_for_portrait_variants() {
    let (_, pool, mut managers, _) = fixture(24);
    deploy(&mut managers, 45004);
    managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
        crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: 10,
            target_uid: EMITTER, buff_id: 31410610, amount: None, params: None, act_info: None })).unwrap();
    let effects = SkillEffectCatalog::from_roots(crate::test_support::game_data(), [31411001], [31410610]);
    assert!(effects.get(31410154).is_some());
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    rate::emit_passive_attack_attributes(&mut modifiers, EMITTER, 31411001, &[],
        rate::RateRuntime { effects: &effects, managers: &managers, pool: &pool,
            context: TargetContext { active_skill_id: 31411001,
                active_skill_source_uid: EMITTER, ..Default::default() } },
        &mut RoundDeterminism::default());
    assert!(modifiers.attack_attributes.contains(&(AttrId::IncantationMight, 150)));
    assert!(!managers.catalog().magic_circle_matches(45004, 40003));
    assert!(!managers.catalog().magic_circle_matches(45004, 30001));
}


#[test]
fn paper_heron_ultimate_gains_eureka_then_scales_damage_and_adds_eight_inspiration() {
    fn run(start: i32) -> (i32, i32, i32) {
        let (fight, _, mut managers, _) = fixture(1);
        managers.execute_gauge(GaugeCommand::new(ORIGIN, inspiration_key(EMITTER),
            GaugeOperation::ChangeValue { delta: -1 })).unwrap();
        managers.eureka.resync_observed(10, 1, start, 10).unwrap();
        let pool = TargetPool::from_fight(&fight).runtime_view(&managers);
        let effects = SkillEffectCatalog::from_roots(crate::test_support::game_data(), [31410131], []);
        let mut invocation: crate::engine::skill::action::SkillInvocation =
            crate::engine::skill::action::SkillRequest { source_uid: 10, skill_id: 31410131 }.into();
        invocation.target = crate::engine::skill::action::SkillTarget::Explicit(-1);
        let result = crate::engine::runtime::drain::run_skill(&mut managers, &pool, &effects,
            &mut RoundDeterminism::default(), TargetContext::default(), invocation, Default::default()).unwrap();
        let damage = result.events.iter().filter_map(|event| match event {
            BattleEvent::HpLost { source_uid: 10, target_uid: -1, amount, .. } => Some(*amount),
            _ => None,
        }).sum();
        (damage, managers.eureka.get(10, 1).current, managers.gauge.get(inspiration_key(EMITTER)).unwrap().current)
    }
    let low = run(2);
    let high = run(6);
    assert_eq!((low.1, low.2), (4, 8));
    assert_eq!((high.1, high.2), (8, 8));
    assert!(low.0 > 0 && high.0 > low.0, "low={low:?} high={high:?}");
}

#[test]
fn paper_heron_emitter_plant_affinity_affects_damage_and_restraint_marker() {
    fn run(plant: bool) -> (i32, bool) {
        let (fight, _, mut managers, effects) = fixture(3);
        if plant {
            managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
                crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: 10,
                    target_uid: EMITTER, buff_id: 31410404, amount: None, params: None, act_info: None })).unwrap();
        }
        let pool = TargetPool::from_fight(&fight).runtime_view(&managers);
        let result = run_impromptu(&mut managers, &pool, &effects, &mut RoundDeterminism::default(),
            TargetContext::default(), 1, EMITTER).unwrap();
        let damage = 10_000_000 - managers.hp.current(-1);
        fn markers(step: &sonettobuf::FightStep, out: &mut Vec<bool>) {
            for effect in &step.act_effect {
                if let Some(hurt) = &effect.hurt_info {
                    if let Some(mark) = hurt.career_restraint { out.push(mark); }
                }
                if let Some(child) = &effect.fight_step { markers(child, out); }
            }
        }
        let mut marks = Vec::new();
        for step in crate::engine::packet::timeline::project(&result.frames).unwrap() { markers(&step, &mut marks); }
        (damage, marks.into_iter().any(|value| value))
    }
    let normal = run(false);
    let plant = run(true);
    assert!(normal.0 > 0 && plant.0 > normal.0, "normal={normal:?}, plant={plant:?}");
    assert!(!normal.1 && plant.1, "normal={normal:?}, plant={plant:?}");
}


#[test]
fn an_an_upgraded_ultimate_grants_spirit_inspiration_and_emitter_might() {
    init_config();
    let entity = |uid, model, team| FightEntityInfo {
        uid: Some(uid), model_id: Some(model), team_type: Some(team), position: Some(uid as i32),
        current_hp: Some(100_000), attr: Some(HeroAttribute { hp: Some(100_000), attack: Some(1000),
            ..Default::default() }), ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam { entitys: vec![
            FightEntityInfo { buffs: vec![BuffInfo { uid: Some(20), buff_id: Some(303901411),
                from_uid: Some(10), layer: Some(3), ..Default::default() }], ..entity(10, 3039, 1) },
            FightEntityInfo { buffs: vec![BuffInfo { uid: Some(21), buff_id: Some(2240000),
                from_uid: Some(11), ..Default::default() }], ..entity(11, 3141, 1) },
        ], ..Default::default() }),
        defender: Some(FightTeam { entitys: vec![entity(-1, 1001, 2)], ..Default::default() }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    assert_eq!(managers.buff.buff_id_amount(10, 303901411), 3);
    let enabled = enable_rule_ops(crate::catalog::impromptu_definition(crate::test_support::game_data()),
        &managers.gauge, &managers.buff.active_features(&managers.hp), EMITTER).pop().unwrap();
    if let RuleOp::Command(BattleCommand::Emitter(command)) = enabled.emitter { managers.execute_emitter(command); }
    for op in [enabled.team_energy, enabled.inspiration] {
        if let RuleOp::Command(BattleCommand::Gauge(command)) = op { managers.execute_gauge(command).unwrap(); }
    }
    let pool = TargetPool::from_fight(&fight).runtime_view(&managers);
    let effects = SkillEffectCatalog::from_roots(crate::test_support::game_data(), [303901351], []);
    let mut invocation: crate::engine::skill::action::SkillInvocation =
        crate::engine::skill::action::SkillRequest { source_uid: 10, skill_id: 303901351 }.into();
    invocation.target = crate::engine::skill::action::SkillTarget::Explicit(-1);
    crate::engine::runtime::drain::run_skill(&mut managers, &pool, &effects,
        &mut RoundDeterminism::default(), TargetContext::default(), invocation, Default::default()).unwrap();
    assert_eq!(managers.gauge.get(inspiration_key(EMITTER)).unwrap().current, 6);
    assert!(managers.buff.has_buff_id(10, 5022));
    assert!(managers.buff.has_buff_id(EMITTER, 303901161));
    assert_eq!(managers.buff.buff_id_amount(EMITTER, 303901161), 5);
    assert!(managers.hp.current(-1) < 100_000);
}

#[test]
fn tooth_fairy_neuralgia_deals_stable_genesis_damage_per_impromptu_attack_and_expires() {
    for (buff_id, rate, expected_count) in [(30530501, 300, 3), (30530503, 500, 5)] {
    let (_, pool, mut managers, effects) = fixture(24);
    deploy(&mut managers, 45004);
    managers.execute_buff(crate::engine::manager::buff::BuffCommand::GrantInternalChild(
        crate::engine::manager::buff::BuffGrantChild { origin: ORIGIN, source_uid: 10,
            target_uid: -1, buff_id, amount: None, params: None,
            act_info: None })).unwrap();
    let result = run_impromptu(&mut managers, &pool, &effects, &mut RoundDeterminism::default(),
        TargetContext::default(), 1, EMITTER).unwrap();
    let hits: Vec<i32> = result.events.iter().filter_map(|event| match event {
        BattleEvent::Hit(hit) if hit.source_uid == EMITTER && hit.skill_id == 31411001
            && hit.damage_from == crate::engine::manager::hp::HurtDamageFromType::Skill => Some(hit.resolved_damage),
        _ => None,
    }).collect();
    let genesis: Vec<i32> = result.outcomes.iter().filter_map(|outcome| match outcome {
        RuleOutcome::Hp(execution) if execution.changes.source_uid == 10 && execution.changes.target_uid == -1 =>
            execution.changes.hp.and_then(|hp| hp.hurt.filter(|hurt| hurt.buff_act_id == 1087).map(|_| -hp.delta)),
        _ => None,
    }).collect();
    assert_eq!(hits.len(), 5);
    assert_eq!(genesis.len(), expected_count);
    assert_eq!(genesis, hits[..expected_count].iter().map(|damage| damage * rate / 1000).collect::<Vec<_>>());
    assert!(!managers.buff.has_buff_id(-1, buff_id));
    }
}
