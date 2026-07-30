use std::collections::HashSet;

use super::*;
use crate::engine::skill::behavior::classify::{BehaviorKey, BehaviorSpec};

#[test]
fn registry_has_unique_exact_keys() {
    let mut keys = HashSet::new();
    for definition in definitions() {
        assert!(keys.insert((definition.key.opcode, definition.key.type_name)));
        assert_eq!(
            super::super::classify::classify(definition.key.opcode, definition.key.type_name,),
            definition.kind
        );
    }
}

#[test]
fn lookup_requires_the_exact_opcode_type_pair() {
    let exact =
        ParsedBehavior::from_spec(BehaviorSpec::new(20002, "AddExPoint"), vec![1], Vec::new());
    let wrong_type = ParsedBehavior::from_spec(
        BehaviorSpec {
            key: BehaviorKey::new(20002, "DelExPoint"),
            kind: BehaviorKind::AddExPoint,
        },
        vec![1],
        Vec::new(),
    );

    assert_eq!(
        find(&exact).map(|definition| definition.phase),
        Some(BehaviorPhase::AfterDamage)
    );
    assert!(find(&wrong_type).is_none());
}

#[test]
fn resource_spend_buff_grant_keeps_its_exact_behavior_key() {
    let definition = find_key(2, "AddBuffPowerUse").unwrap();

    assert_eq!(definition.kind, BehaviorKind::AddBuffPowerUse);
    assert_eq!(definition.phase, BehaviorPhase::AfterDamage);
    assert!(definition.destination);
    assert!(find_key(2, "AddBuff").is_none());
    assert!(find_key(1, "AddBuffPowerUse").is_none());
}

#[test]
fn implemented_skill_casts_own_destinations_but_unimplemented_siblings_do_not() {
    for (opcode, type_name) in [
        (50008, "DirectUseSkill"),
        (50012, "DirectUseSkillNoAct"),
        (50038, "DirectUseSkillNoAct2"),
        (50010, "DirectUseGroupAndStarSkill"),
        (60188, "ConsumePowerUseSkill"),
        (60225, "RandomUseSkill"),
    ] {
        assert!(find_key(opcode, type_name).unwrap().destination);
    }
    assert!(find_key(60053, "DirectUseSkill2").unwrap().destination);
}

#[test]
fn destination_readiness_belongs_to_the_exact_registry_row() {
    let root_add =
        ParsedBehavior::from_spec(BehaviorSpec::new(1, "AddBuff"), vec![100, 1], Vec::new());
    let specialized_add = ParsedBehavior::from_spec(
        BehaviorSpec::new(20005, "AddBuffRound"),
        vec![100, 1],
        Vec::new(),
    );

    assert!(find(&root_add).unwrap().destination);
    assert_eq!(
        find(&root_add).unwrap().fire_count_mode,
        FireCountMode::Transfer
    );
    assert!(find(&specialized_add).unwrap().destination);
    assert_eq!(
        find(&specialized_add).unwrap().fire_count_mode,
        FireCountMode::Transfer
    );

    for (opcode, type_name) in [
        (20010, "Bloodlust"),
        (60242, "CrystalReuse"),
        (60243, "CrystalAddSkillRate"),
        (60244, "CrystalAddCardRank"),
    ] {
        assert!(find_key(opcode, type_name).unwrap().destination);
    }
}

#[test]
fn bloodlust_requires_one_positive_damage_rate() {
    let definition = find_key(20010, "Bloodlust").unwrap();
    let supports = definition.supports.unwrap();
    let behavior = |args| ParsedBehavior::new(20010, "Bloodlust", args);

    assert!(supports(&behavior(vec![300])));
    assert!(!supports(&behavior(Vec::new())));
    assert!(!supports(&behavior(vec![300, 1])));
    assert!(!supports(&behavior(vec![0])));
    assert!(!supports(&behavior(vec![-1])));
}

#[test]
fn resource_driven_behaviors_validate_their_configured_operands() {
    let supports = |opcode, type_name, args, raw_args| {
        let behavior =
            ParsedBehavior::from_spec(BehaviorSpec::new(opcode, type_name), args, raw_args);
        find(&behavior).unwrap().supports.unwrap()(&behavior)
    };

    assert!(supports(
        60142,
        "ConsumePowerAddBuff",
        vec![2],
        vec!["2".into(), "31170011".into()]
    ));
    assert!(!supports(
        60142,
        "ConsumePowerAddBuff",
        vec![2],
        vec!["2".into(), "".into()]
    ));
    assert!(supports(60152, "AddEmitterEnergy", vec![6], Vec::new()));
    assert!(!supports(60152, "AddEmitterEnergy", vec![-1], Vec::new()));
    assert!(supports(
        60187,
        "AddPowerByCritCount",
        vec![2, 1],
        Vec::new()
    ));
    assert!(supports(
        60188,
        "ConsumePowerUseSkill",
        vec![2, 311701210],
        Vec::new()
    ));
    assert!(supports(
        60189,
        "AddEnergyToCard",
        vec![1, -1, 1],
        Vec::new()
    ));
    assert!(!supports(
        60189,
        "AddEnergyToCard",
        vec![1, 1, 1],
        Vec::new()
    ));
}

#[test]
fn add_team_energy_is_parent_owned_only_during_setup() {
    let definition = find_key(60153, "AddTeamEnergy").unwrap();

    assert_eq!(definition.output_owner, OutputOwner::SetupParent);
    assert_eq!(
        definition.output_owner.resolve(false, true),
        OutputOwner::Parent
    );
    assert_eq!(
        definition.output_owner.resolve(false, false),
        OutputOwner::Skill
    );
}

#[test]
fn conduit_ex_point_is_parent_owned_only_during_setup() {
    let definition = find_key(60292, "AddDeviceExPoint").unwrap();

    assert_eq!(definition.output_owner, OutputOwner::SetupParent);
    assert_eq!(
        definition.output_owner.resolve(false, true),
        OutputOwner::Parent
    );
    assert_eq!(
        definition.output_owner.resolve(false, false),
        OutputOwner::Skill
    );
}

#[test]
fn readiness_requires_and_runs_the_exact_argument_validator() {
    let valid = ParsedBehavior::from_spec(
        BehaviorSpec::new(60259, "SupplyShield2"),
        vec![31270012, 102, 1800, 0, 102, 6500, 201, 900],
        Vec::new(),
    );
    let malformed = ParsedBehavior::from_spec(
        BehaviorSpec::new(60259, "SupplyShield2"),
        vec![31270012, 102, 1800],
        Vec::new(),
    );
    let shield = find(&valid).unwrap();
    let add_buff = find_key(1, "AddBuff").unwrap();

    assert!(shield.supports.is_some_and(|supports| supports(&valid)));
    assert!(!shield.supports.is_some_and(|supports| supports(&malformed)));
    assert!(
        add_buff
            .supports
            .is_some_and(|supports| supports(&ParsedBehavior::new(1, "AddBuff", vec![123])))
    );
    assert!(
        !add_buff
            .supports
            .is_some_and(|supports| supports(&ParsedBehavior::new(1, "AddBuff", Vec::new())))
    );
}

#[test]
fn configured_skill_without_a_target_override_emits_once_per_slot() {
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(50012, "DirectUseSkillNoAct"),
        vec![434725, 1],
        Vec::new(),
    );

    assert_eq!(
        find(&behavior).unwrap().target_emission_mode,
        TargetEmissionMode::Once
    );
}
