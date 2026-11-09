use super::*;

#[test]
fn talent_choices_light_whole_branches_like_the_captured_requests() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let points = config::configs::get().talent_points(4);

    let extra = light_talents("", 2, 3, points).unwrap();
    assert_eq!(extra, "2#21,22,23");
    let extra = cancel_talents(&extra, 2, 2).unwrap();
    assert_eq!(extra, "2#21");
    let extra = light_talents(&extra, 2, 3, points).unwrap();
    let extra = light_talents(&extra, 1, 2, points).unwrap();
    assert_eq!(extra, "1#11,12|2#21,22,23");
    assert_eq!(
        cancel_talents(&extra, 2, 3),
        None,
        "full branch locked while another has talents"
    );

    assert_eq!(
        light_talents("2#21", 1, 1, points),
        None,
        "second branch before a full one"
    );
    assert_eq!(
        light_talents("", 2, 3, config::configs::get().talent_points(2)),
        Some("2#21,22,23".to_owned())
    );
    assert_eq!(
        light_talents("", 2, 3, config::configs::get().talent_points(1)),
        None,
        "no points below rank 2"
    );
}
