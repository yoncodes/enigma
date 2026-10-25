//! Timeline projection regression groups.

use sonettobuf::{
    CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, effect_type_enum::EffectType,
};

use super::*;
use crate::engine::{
    manager::{
        BattleManagers,
        buff::BuffMarkerResult,
        card::{CardAddTemporary, CardCommand, CardManager},
        entity::{EntityChanges, EntityOperation},
        ex_point::{ExPointApplyResult, ExPointChanges, ExPointKind},
        field::{FieldCommand, FieldDefinition, FieldManager, FieldOperation, FieldThreshold},
        gauge::{GaugeChange, GaugeChangeKind, GaugeKey},
        hp::{
            DamageEffectKind, DamageRecord, HpChange, HpChanges, HurtDamageFromType, HurtInfoData,
            MaxHpChange, ShieldChange,
        },
        injury::InjuryChange,
        shield::ShieldCommand,
        summon::{SummonCommand, SummonManager},
        upgrade::{UpgradeApplied, UpgradeCommand, UpgradeManager, UpgradeOperation},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

mod cards;
mod counters;
mod entities;
mod field_and_wave;
mod frames;
mod gauges;
mod progress;
mod shield;
mod temporary_cards;
mod upgrade;
