use super::BuffActDefinition;

mod combat;
mod effects;
mod lingering_glow;
mod resources;
mod support;
mod team;

const GROUPS: &[&[BuffActDefinition]] = &[
    effects::DEFINITIONS,
    combat::DEFINITIONS,
    team::DEFINITIONS,
    resources::DEFINITIONS,
    lingering_glow::DEFINITIONS,
    support::DEFINITIONS,
];

const COUNT: usize = {
    let mut count = 0;
    let mut group = 0;
    while group < GROUPS.len() {
        count += GROUPS[group].len();
        group += 1;
    }
    count
};

pub(super) static DEFINITIONS: [BuffActDefinition; COUNT] = {
    let mut definitions = [GROUPS[0][0]; COUNT];
    let mut offset = 0;
    let mut group = 0;
    while group < GROUPS.len() {
        let entries = GROUPS[group];
        let mut index = 0;
        while index < entries.len() {
            definitions[offset + index] = entries[index];
            index += 1;
        }
        offset += entries.len();
        group += 1;
    }
    definitions
};
