use super::attacker::{Attacker, BattleRoster};
use crate::engine::fight::{defender::Defender, versions};
use anyhow::Result;
use sonettobuf::{Fight, FightGroup, FightTaskBox, fight::FightActType};

pub struct BuiltFight {
    pub fight: Fight,
    pub ex_attributes: Vec<(i64, sonettobuf::HeroExAttribute)>,
    pub sp_attributes: Vec<(i64, sonettobuf::HeroSpAttribute)>,
    pub battle_rule_skills: Vec<crate::engine::fight::rules::OwnedBattleSkill>,
}

#[derive(Clone, Copy, Default)]
pub struct FightOptions {
    pub is_balance: bool,
    pub use_record: bool,
}

pub fn build_fight(
    roster: &BattleRoster,
    episode_id: i32,
    battle_id: i32,
    fight_group: &FightGroup,
    options: FightOptions,
    params: Option<&str>,
) -> Result<BuiltFight> {
    let mut attacker = Attacker::get(
        roster,
        episode_id,
        battle_id,
        options.is_balance,
        fight_group,
        params,
    )?;
    let defender_uid_offset = attacker.reserved_uid_offset;
    let mut defender = Defender::get(battle_id, defender_uid_offset)?;
    attacker.team.sp_entitys = defender.attacker_sp_entitys;
    attacker.team.sp_fight_entities = defender.attacker_sp_fight_entities;
    apply_battle_rules(
        episode_id,
        battle_id,
        &mut attacker.team,
        &mut defender.team,
    )?;

    Ok(BuiltFight {
        fight: Fight {
            attacker: Some(attacker.team),
            defender: Some(defender.team),
            cur_round: Some(1),
            max_round: Some(defender.max_round),
            is_finish: Some(false),
            cur_wave: Some(1),
            battle_id: Some(battle_id),
            version: Some(versions::current()?),
            is_record: Some(options.use_record),
            episode_id: Some(episode_id),
            fight_act_type: Some(FightActType::Normal.into()),
            last_change_hero_uid: Some(0),
            progress: Some(0),
            progress_max: Some(0),
            fight_task_box: Some(FightTaskBox { tasks: vec![] }),
            ..Default::default()
        },
        ex_attributes: attacker.ex_attributes,
        sp_attributes: attacker.sp_attributes,
        battle_rule_skills: Vec::new(),
    })
}

fn apply_battle_rules(
    episode_id: i32,
    battle_id: i32,
    attacker: &mut sonettobuf::FightTeam,
    defender: &mut sonettobuf::FightTeam,
) -> Result<()> {
    let fight = sonettobuf::Fight {
        episode_id: Some(episode_id),
        battle_id: Some(battle_id),
        ..Default::default()
    };
    let mut attacker_rules = Vec::new();
    let mut defender_rules = Vec::new();
    for rule in crate::engine::fight::rules::configured(&fight) {
        if rule.rule_type == crate::engine::fight::rules::AdditionRuleType::FightSkill {
            continue;
        }
        match rule.side {
            crate::engine::fight::rules::BattleRuleSide::Attacker => {
                attacker_rules.push(rule.skill_id)
            }
            crate::engine::fight::rules::BattleRuleSide::Defender => {
                defender_rules.push(rule.skill_id)
            }
            crate::engine::fight::rules::BattleRuleSide::Both => {
                attacker_rules.push(rule.skill_id);
                defender_rules.push(rule.skill_id);
            }
        }
    }

    for entity in attacker
        .entitys
        .iter_mut()
        .chain(&mut attacker.sub_entitys)
        .chain(&mut attacker.sp_entitys)
        .chain(&mut attacker.sp_fight_entities)
    {
        entity.passive_skill.extend(attacker_rules.iter().copied());
    }
    for entity in defender
        .entitys
        .iter_mut()
        .chain(&mut defender.sub_entitys)
        .chain(&mut defender.sp_entitys)
        .chain(&mut defender.sp_fight_entities)
    {
        entity
            .passive_skill
            .splice(0..0, defender_rules.iter().copied());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_battle_rules;

    #[test]
    fn tower_battle_rules_keep_their_configured_side_and_order() {
        crate::test_support::init_config();
        let entity = || sonettobuf::FightEntityInfo {
            passive_skill: vec![99],
            ..Default::default()
        };
        let mut attacker = sonettobuf::FightTeam {
            entitys: vec![entity()],
            ..Default::default()
        };
        let mut defender = sonettobuf::FightTeam {
            entitys: vec![entity()],
            ..Default::default()
        };

        apply_battle_rules(90002501, 9000303, &mut attacker, &mut defender).unwrap();

        assert_eq!(
            attacker.entitys[0].passive_skill,
            vec![99, 22_301_961, 90_120_002]
        );
        assert_eq!(
            defender.entitys[0].passive_skill,
            vec![370_003_003, 22_301_962, 370_003_013, 99]
        );
    }
}
