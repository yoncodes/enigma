use std::collections::BTreeSet;

#[derive(Debug)]
pub(crate) struct Subject {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) slug: String,
    pub(crate) variants: Vec<Variant>,
    pub(crate) chapter_id: Option<i32>,
}

#[derive(Debug)]
pub(crate) struct Variant {
    pub(crate) label: String,
    pub(crate) source_id: Option<i32>,
    pub(crate) source_rank: Option<i32>,
    pub(crate) observation: &'static str,
    pub(crate) scan: Scan,
}

#[derive(Debug)]
pub(crate) struct Scan {
    pub(crate) skills: Vec<Skill>,
    pub(crate) buffs: Vec<Buff>,
    pub(crate) errors: BTreeSet<String>,
    pub(crate) warnings: BTreeSet<String>,
    pub(crate) gaps: usize,
}

impl Scan {
    pub(crate) fn status(&self) -> &'static str {
        if self.errors.is_empty() && self.gaps == 0 {
            "semantically ready"
        } else {
            "incomplete"
        }
    }
}

#[derive(Debug)]
pub(crate) struct Skill {
    pub(crate) id: i32,
    pub(crate) effect_id: i32,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) mechanic_template: String,
    pub(crate) effect_metadata: String,
    pub(crate) art_description: String,
    pub(crate) observed: bool,
    pub(crate) slots: Vec<Slot>,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Slot {
    pub(crate) number: usize,
    pub(crate) behavior: Node,
    pub(crate) conditions: Vec<Node>,
    pub(crate) behavior_target: String,
    pub(crate) condition_target: String,
    pub(crate) limit: i32,
    pub(crate) round_limit: i32,
    pub(crate) route: String,
    pub(crate) referenced_skills: Vec<i32>,
    pub(crate) referenced_buffs: Vec<i32>,
}

#[derive(Debug)]
pub(crate) struct Node {
    pub(crate) opcode: Option<i32>,
    pub(crate) type_name: String,
    pub(crate) raw: String,
    pub(crate) registry: &'static str,
    pub(crate) semantic: &'static str,
    pub(crate) detail: String,
    pub(crate) observation: &'static str,
}

#[derive(Debug)]
pub(crate) struct Buff {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) duration: i32,
    pub(crate) type_id: i32,
    pub(crate) observed: bool,
    pub(crate) acts: Vec<BuffAct>,
}

#[derive(Debug)]
pub(crate) struct BuffAct {
    pub(crate) node: Node,
    pub(crate) effect_time: i32,
    pub(crate) event: String,
    pub(crate) destination: String,
    pub(crate) owns_duration: bool,
    pub(crate) wire: &'static str,
}
