use sonettobuf::{BeginRoundOper, Fight, FightDeviceOper};

use crate::engine::{
    manager::{BattleManagers, conduit::ConduitCommand},
    round::{command::commands_from_opers, state::RoundState},
    runtime::{determinism::RoundDeterminism, drain, schedule},
    skill::{
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        target::{TargetContext, TargetPool},
    },
};

pub(super) struct Simulation<'a> {
    pub fight: Fight,
    pub managers: BattleManagers,
    pub pool: TargetPool,
    pub catalog: &'a SkillEffectCatalog,
    pub determinism: RoundDeterminism,
    pub context: TargetContext,
}

impl<'a> Simulation<'a> {
    pub fn new(
        fight: &Fight,
        managers: &BattleManagers,
        catalog: &'a SkillEffectCatalog,
        round: &RoundState,
        determinism: &RoundDeterminism,
    ) -> Self {
        let mut round = round.clone();
        round.begin_round();
        let mut fight = fight.clone();
        fight.cur_round = Some(round.cur_round);
        if let Some(team) = fight.attacker.as_mut() {
            for skill in &mut team.skill_infos {
                skill.cd = Some(skill.cd.unwrap_or_default().saturating_sub(1).max(0));
            }
        }
        let mut managers = managers.clone();
        managers.begin_action_round();
        Self {
            context: TargetContext {
                battle_id: fight.battle_id.unwrap_or_default(),
                current_round: round.cur_round,
                ..Default::default()
            },
            pool: TargetPool::from_fight_with_catalog(managers.catalog(), &fight),
            fight,
            managers,
            catalog,
            determinism: determinism.clone(),
        }
    }

    pub fn player(
        &self,
        opers: &[BeginRoundOper],
        devices: &[FightDeviceOper],
    ) -> Option<(BattleManagers, RoundDeterminism)> {
        let mut managers = self.managers.clone();
        let mut determinism = self.determinism.clone();
        let execute = || -> Result<_, drain::DrainError> {
            let selection = drain::run_command_group(
                &mut managers,
                &self.pool,
                self.catalog,
                &mut determinism,
                self.context,
                devices.iter().filter_map(|operation| {
                    Some(RuleOp::Command(BattleCommand::Conduit(
                        ConduitCommand::SelectGroup {
                            source_uid: operation.uid?,
                            group: operation.index?,
                        },
                    )))
                }),
            )?;
            schedule::run_player_phase(
                &self.fight,
                &mut managers,
                &self.pool,
                self.catalog,
                &mut determinism,
                self.context,
                commands_from_opers(opers),
                selection.frames,
                1,
                crate::engine::manager::emitter::UID,
            )?;
            Ok((managers, determinism))
        };
        execute()
            .inspect_err(|error| tracing::warn!(?error, "auto-battle rejected player proposal"))
            .ok()
    }
}
