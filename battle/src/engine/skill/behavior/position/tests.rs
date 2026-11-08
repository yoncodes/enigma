use super::*;

#[test]
fn move_front_requires_one_positive_distance() {
    assert!(Handler::supports(&ParsedBehavior::new(
        60061,
        "MoveFront",
        vec![2],
    )));
    for args in [vec![], vec![0], vec![-1], vec![5], vec![98], vec![1, 2]] {
        assert!(!Handler::supports(&ParsedBehavior::new(
            60061,
            "MoveFront",
            args,
        )));
    }
}
