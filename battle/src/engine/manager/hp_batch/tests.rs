use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    damage::handler::resolve_avoided_attack_command,
    manager::hp::{DamageEffectKind, HpLoss, HurtDamageFromType},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn a_later_linked_hit_cannot_damage_an_absorber_after_the_target_dies() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(2), entity(3)],
            ..Default::default()
        }),
        ..Default::default()
    });
    let origin = CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1147, "FoundationCounter"),
    };
    let HpCommand::Damage(mut damage) = resolve_avoided_attack_command(1, 2, origin) else {
        panic!("expected damage");
    };
    damage.amount = 100;
    damage.effect_kind = DamageEffectKind::Normal;
    damage.hurt.hurt_effect_type =
        sonettobuf::effect_type_enum::EffectType::Additionaldamage as i32;
    damage.hurt.damage_from = HurtDamageFromType::Additional;

    let mut later = damage;
    later.amount = 30;
    let absorbed = HpCommand::Lose(HpLoss {
        origin,
        source_uid: 1,
        target_uid: 3,
        amount: 20,
        config_effect: -1,
        hurt: None,
    });

    let changes = managers
        .execute_linked_damage(vec![
            LinkedHit {
                target_uid: 2,
                commands: vec![HpCommand::Damage(damage)],
            },
            LinkedHit {
                target_uid: 2,
                commands: vec![absorbed, HpCommand::Damage(later)],
            },
        ])
        .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(managers.hp.current(2), 0);
    assert_eq!(managers.hp.current(3), 100);
}
