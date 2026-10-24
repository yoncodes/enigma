use sonettobuf::{
    BuffActInfo, BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute,
    HeroSpAttribute, PowerInfo,
};

use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector, CommandOrigin},
        conduit::ConduitCommand,
        ex_point::{
            ExPointChange, ExPointChanges, ExPointCommand, ExPointConfigureSynchronization,
            ExPointRecordSynchronizationAction, SynchronizationDefinition,
        },
        gauge::{GaugeCommand, GaugeOperation},
        hp::{DamageEffectKind, HpCommand, HpDamage, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

use super::{BattleManagers, base_hero_sp_attribute};

mod projection_and_upgrade;
mod resources_and_limits;
