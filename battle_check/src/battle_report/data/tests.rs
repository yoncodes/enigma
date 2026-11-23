use super::*;
use crate::battle_report::model::StageRoute;

#[test]
fn writes_versioned_subject_modules() {
    let root =
        std::env::temp_dir().join(format!("enigma-battle-report-data-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let subjects = [
        Subject {
            id: 42,
            name: "Example".to_owned(),
            slug: "example".to_owned(),
            variants: Vec::new(),
            chapter_id: None,
            route: Some(StageRoute {
                chapter_name: Some("Focused Flashback".to_owned()),
                episode_name: Some("Psych Trauma III".to_owned()),
            }),
        },
        Subject {
            id: 43,
            name: "Episode 43".to_owned(),
            slug: "episode-43".to_owned(),
            variants: Vec::new(),
            chapter_id: None,
            route: Some(StageRoute {
                chapter_name: None,
                episode_name: None,
            }),
        },
    ];
    let mut generated = Vec::new();
    let mut reports = BTreeMap::new();

    write_subjects(&root, "stage", &subjects, &mut generated, &mut reports).unwrap();
    write_manifest(&root, reports, &mut generated).unwrap();

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(ROOT).join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["schema_version"], 3);
    assert_eq!(manifest["reports"]["stage"][0], "stage/42.json");
    let subject: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join(ROOT).join("stage").join("42.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(subject["route"]["chapter_name"], "Focused Flashback");
    assert_eq!(subject["route"]["episode_name"], "Psych Trauma III");
    let unnamed: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join(ROOT).join("stage").join("43.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(unnamed["route"], serde_json::json!({}));

    fs::remove_dir_all(root).unwrap();
}
