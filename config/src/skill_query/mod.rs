use crate::GameDB;

impl GameDB {
    pub fn skill_target_type(&self, logic_target: &str) -> Option<i32> {
        let code = logic_target.split('#').next()?;
        self.skill_target_type_define
            .iter()
            .find(|row| row.logic_target == code)
            .map(|row| row.define)
    }
}

#[cfg(test)]
mod tests;
