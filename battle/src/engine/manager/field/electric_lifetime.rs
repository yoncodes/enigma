use crate::{
    catalog::BattleCatalog,
    engine::skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

use super::{FieldCommand, FieldManager, FieldOperation};

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "ElectricFieldDuration"),
};

impl FieldManager {
    pub(crate) fn electric_round_end(&self, catalog: BattleCatalog) -> Vec<FieldCommand> {
        let mut commands = self
            .states()
            .filter(|field| catalog.is_electric_field(field.definition.field_id))
            .filter_map(|field| {
                let operation = match field.definition.duration {
                    1 => FieldOperation::Remove,
                    2.. => FieldOperation::ChangeDuration { delta: -1 },
                    _ => return None,
                };
                Some(FieldCommand {
                    origin: ORIGIN,
                    team: field.team,
                    operation,
                })
            })
            .collect::<Vec<_>>();
        commands.sort_by_key(|command| command.team);
        commands
    }
}
