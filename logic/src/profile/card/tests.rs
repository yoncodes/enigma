use super::{valid_layout, valid_show_settings};

#[test]
fn validates_player_card_wire_formats() {
    assert!(valid_show_settings(&["1#2".into(), "2#0".into()]));
    assert!(!valid_show_settings(&["1#2".into(), "1#1".into()]));
    assert!(valid_layout("1#3|2#5", 1..=5, |id| matches!(id, 3 | 5)));
    assert!(!valid_layout("1#3|1#5", 1..=5, |_| true));
}
