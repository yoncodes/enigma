use super::*;

#[test]
fn lifetime_reward_bits_match_client_marker_layout() {
    let mark = (1..=9)
        .filter_map(lifetime_reward_bit)
        .fold(0, |mark, bit| mark | bit);
    assert_eq!(mark, 1022);
    assert_eq!(lifetime_reward_bit(-1), None);
    assert_eq!(lifetime_reward_bit(32), None);
}

#[test]
fn live_monday_capture_uses_first_sign_in_bonus() {
    assert_eq!(sign_in_bonus_id(1_784_590_847_043), 1);
}
