mod fight;

pub(crate) use fight::system_plan_rule_skills;
pub use fight::{apply_assist_boss, system_plan_talents};

#[cfg(test)]
mod test;
