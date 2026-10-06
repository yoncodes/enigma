use sonettobuf::{BeginRoundReply, BeginRoundRequest, StartDungeonReply};

use crate::Battle;

mod attacker;
mod start;

pub use crate::engine::entity::input::{EquipmentBuildInput, HeroBuildInput};
pub use attacker::{
    BattleFighter, BattleRoster, BattleRosterPlan, ComposeSupportLookup, plan_roster,
};
pub use start::{BuiltFight, FightOptions, build_fight};

pub fn start_reply(battle: &Battle) -> StartDungeonReply {
    let (fight, round) = battle.runtime().start_state();
    StartDungeonReply {
        fight: Some(fight.clone()),
        round: round.cloned(),
    }
}

pub fn begin_round(
    battle: &mut Battle,
    request: BeginRoundRequest,
) -> Result<BeginRoundReply, String> {
    Ok(BeginRoundReply {
        round: Some(battle.runtime_mut().advance_round(request)?),
    })
}
