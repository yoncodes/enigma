use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use sonettobuf::{Fight, FightDeviceAreaInfo, FightEntityInfo, FightTeam};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(60291, "AddDevicePower"),
};

mod activation;
mod configuration;
mod spending;
