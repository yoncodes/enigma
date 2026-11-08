use super::gallery_points;

#[test]
fn act128_gallery_config_pairs_captured_stage_and_thresholds() {
    crate::init_test_config();
    let tables = crate::configs::get();

    assert_eq!(tables.activity128_boss_type(13809, 1), Some(109));

    let row = tables.activity128_gallery(109).unwrap();
    assert_eq!(row.r#type, 109);
    let points = tables.activity128_gallery_points(109).unwrap();
    assert_eq!(
        points,
        vec![
            (2_500_000, 200),
            (5_000_000, 200),
            (7_500_000, 50),
            (10_000_000, 50),
        ]
    );
    assert_eq!(
        points
            .iter()
            .filter(|(threshold, _)| *threshold <= 8_427_039)
            .map(|(_, award)| award)
            .sum::<i32>(),
        450
    );

    let mut malformed = row.clone();
    malformed.point_exp = "200|200|50".to_owned();
    assert_eq!(gallery_points(&malformed), None);
}
