use sonettobuf::{BeginRoundReply, BeginRoundRequest, StartDungeonReply};

use crate::engine::runtime::BattleRuntime;

mod attacker;
mod start;

pub use attacker::{
    BattleFighter, BattleRoster, BattleRosterPlan, ComposeSupportLookup, plan_roster,
};
pub use start::{BuiltFight, FightOptions, build_fight};

pub fn start_reply(runtime: &BattleRuntime) -> StartDungeonReply {
    let (fight, round) = runtime.start_state();
    StartDungeonReply {
        fight: Some(fight.clone()),
        round: round.cloned(),
    }
}

pub fn begin_round(
    runtime: &mut BattleRuntime,
    request: BeginRoundRequest,
) -> Result<BeginRoundReply, String> {
    Ok(BeginRoundReply {
        round: Some(runtime.advance_round(request)?),
    })
}
