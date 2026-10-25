use super::*;

#[test]
fn builds_every_configured_entry_and_uses_owned_hero_time() {
    let tables = config::GameDB::load("../data/excel2json").unwrap();
    let hero_times = HashMap::from([(3023, 123_456)]);

    let reply = build_info(&tables, &hero_times);

    assert_eq!(reply.udimos.len(), tables.udimo.len());
    assert_eq!(reply.backgrounds.len(), tables.udimo_background.len());
    assert_eq!(reply.decorations.len(), tables.udimo_decoration.len());
    assert_eq!(
        reply
            .udimos
            .iter()
            .find(|entry| entry.udimo_id == Some(360008))
            .and_then(|entry| entry.get_time),
        Some(123_456)
    );
}
