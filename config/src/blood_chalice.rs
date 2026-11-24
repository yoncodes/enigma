use anyhow::Context;

use crate::fight_xcjl_const::FightXcjlConstTable;

const HP_PER_POINT: i32 = 3;
const ATTACKED_HP_RATE: i32 = 5;

pub struct Rules {
    pub hp_per_point: i32,
    pub attacked_hp_rate: i32,
}

impl Rules {
    pub(crate) fn from_table(table: &FightXcjlConstTable) -> anyhow::Result<Self> {
        Ok(Self {
            hp_per_point: positive_value(table, HP_PER_POINT)?,
            attacked_hp_rate: positive_value(table, ATTACKED_HP_RATE)?,
        })
    }
}

fn positive_value(table: &FightXcjlConstTable, id: i32) -> anyhow::Result<i32> {
    table
        .get(id)
        .and_then(|row| row.value.trim().parse().ok())
        .filter(|value| *value > 0)
        .with_context(|| format!("fight_xcjl_const row {id} must define a positive integer"))
}
