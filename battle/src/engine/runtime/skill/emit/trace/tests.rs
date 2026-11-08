use super::*;
use crate::engine::skill::{
    action::{SkillPhase, SkillRequest},
    effect::catalog,
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

#[test]
fn target_record_preserves_selection_identity_and_health_provenance() {
    crate::test_support::init_config();
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                position: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let entity = &mut fight.attacker.as_mut().unwrap().entitys[0];
    entity.current_hp = Some(50);
    entity.attr.as_mut().unwrap().hp = Some(200);
    let managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 430811,
    }
    .into();
    invocation.phase = Some(SkillPhase::AfterHit);
    let slot = &catalog::global().get(430811).unwrap().slots[0];
    let report = record(
        &invocation,
        slot,
        &SkillOpTrigger::Active,
        TargetContext::default(),
        &managers,
        &pool,
        &[10],
    );

    assert_eq!(report["skill"], 430811);
    assert_eq!(report["effect"], 430811);
    assert_eq!(report["slot"], 1);
    assert_eq!(report["behavior"]["opcode"], 20016);
    assert_eq!(report["behavior"]["args"], json!(["0", "102", "240"]));
    assert_eq!(report["target"]["code"], 107);
    assert_eq!(report["target"]["raw"], json!([]));
    assert_eq!(report["phase"], "AfterHit");
    assert_eq!(report["selected"], json!([10]));
    assert_eq!(
        report["allies"][0]["pool"],
        json!({"current": 100, "max": 100})
    );
    assert_eq!(
        report["allies"][0]["manager"],
        json!({"current": 50, "max": 200})
    );
}
