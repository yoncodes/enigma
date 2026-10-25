use super::*;
use crate::engine::manager::card::CardSetup;
use sonettobuf::{
    BeginRoundOper, BeginRoundRequest, FightEntityInfo, FightTeam, UseClothSkillRequest,
};

mod absorb_layout;
mod auto_battle;
mod cards;
mod cloth;
mod core;
mod qte;
mod rounds;
mod terminal;

fn runtime(fight: Fight) -> BattleRuntime {
    BattleRuntime::new(
        crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
        fight,
    )
}
