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
            ExPointRecordSynchronizationAction, ExPointTransfer, SynchronizationDefinition,
        },
        gauge::{GaugeCommand, GaugeOperation},
        hp::{DamageEffectKind, HpCommand, HpDamage, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::rule::{DefinitionKey, RuleDomain},
};

use super::{BattleManagers, base_hero_sp_attribute};

mod each_change_attr;
mod emitter;
mod indicator;
mod projection_and_upgrade;
mod resources_and_limits;
