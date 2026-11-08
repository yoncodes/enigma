mod exploration;

#[test]
fn character_voice_types_distinguish_summon_from_skin_unlocks() {
    crate::init_test_config();
    let tables = crate::get();
    assert_eq!(
        tables
            .character_voices_of_type(3136, crate::CharacterVoiceType::Summon)
            .map(|row| row.audio)
            .collect::<Vec<_>>(),
        vec![1_313_601]
    );
    assert_eq!(
        tables
            .character_voices_of_type(3136, crate::CharacterVoiceType::GetSkin)
            .map(|row| row.audio)
            .collect::<Vec<_>>(),
        vec![1_313_633]
    );
}
