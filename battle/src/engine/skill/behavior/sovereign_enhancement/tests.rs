use super::*;
use crate::engine::skill::{action::SkillPhase, behavior, effect::SkillEffectCatalog};

#[test]
fn exact_slots_preserve_their_grammar_and_action_start_route() {
    crate::test_support::init_config();
    let catalog = SkillEffectCatalog::from_roots(
        config::configs::get(),
        [31470131, 31471131, 31472131, 31473131, 31475131],
        [],
    );
    for id in [31470131, 31471131, 31472131, 31473131, 31475131] {
        for slot in catalog.get(id).unwrap().slots.iter().take(4) {
            assert!(behavior::is_slot_supported(slot));
            assert_eq!(
                slot.active_phases().unwrap(),
                vec![SkillPhase::SourceOpening]
            );
            let mut invalid = slot.behavior.clone();
            invalid.raw_args.push("0".into());
            assert!(!behavior::is_supported(&invalid));
            invalid = slot.behavior.clone();
            invalid.spec.key.opcode += 1;
            assert!(!behavior::is_supported(&invalid));
            invalid = slot.behavior.clone();
            invalid.raw_args[1] = "4:102_80|211_120".into();
            assert!(!behavior::is_supported(&invalid));
        }
    }
}
