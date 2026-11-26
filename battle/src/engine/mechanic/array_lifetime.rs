use crate::catalog::BattleCatalog;
use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector, array::ArrayBuffOwner},
        field::{FieldCommand, FieldOperation, FieldState},
    },
    skill::{
        behavior::{classify::BehaviorKind, registry},
        rule::{
            CommandOrigin, DefinitionKey, RuleDomain,
            output::{BattleCommand, RuleOp},
        },
    },
};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "ArrayDuration"),
};

pub(crate) fn is_timed(field: FieldState, catalog: BattleCatalog) -> bool {
    let key = field.origin.key;
    field.definition.duration > 0
        && field.origin.domain == RuleDomain::Behavior
        && registry::find_key(key.opcode, key.type_name)
            .is_some_and(|definition| definition.kind == BehaviorKind::AddMagicCircle)
        && catalog.uses_array_duration(field.definition.field_id)
}

pub(crate) fn advance(managers: &BattleManagers, team: i32) -> Vec<RuleOp> {
    let Some(field) = managers.field.get(team) else {
        return Vec::new();
    };
    if !is_timed(field, managers.catalog()) {
        return Vec::new();
    }
    if field.definition.duration > 1 {
        return vec![RuleOp::Command(BattleCommand::Field(FieldCommand {
            origin: ORIGIN,
            team,
            operation: FieldOperation::ChangeDuration { delta: -1 },
        }))];
    }
    let mut ops = grants_to_remove(managers, field, ORIGIN)
        .map(|remove| RuleOp::Command(BattleCommand::Buff(BuffCommand::ExpireArrayBuff(remove))))
        .collect::<Vec<_>>();
    ops.push(remove_field(team, ORIGIN));
    ops
}

fn grants_to_remove(
    managers: &BattleManagers,
    field: FieldState,
    origin: CommandOrigin,
) -> impl Iterator<Item = BuffRemove> + '_ {
    let owner = ArrayBuffOwner {
        team: field.team,
        field_id: field.definition.field_id,
        caster_uid: field.owner_uid,
    };
    managers
        .buff
        .array_buff_uids(owner)
        .map(move |(target_uid, buff_uid)| BuffRemove {
            origin,
            target_uid,
            selector: BuffRemoveSelector::Uid(buff_uid),
        })
}

pub(crate) fn withdraw(
    managers: &BattleManagers,
    field: FieldState,
    origin: CommandOrigin,
) -> Vec<RuleOp> {
    let mut ops = grants_to_remove(managers, field, origin)
        .map(|remove| RuleOp::Command(BattleCommand::Buff(BuffCommand::RemoveAfterTrigger(remove))))
        .collect::<Vec<_>>();
    ops.push(remove_field(field.team, origin));
    ops
}

fn remove_field(team: i32, origin: CommandOrigin) -> RuleOp {
    RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin,
        team,
        operation: FieldOperation::Remove,
    }))
}
