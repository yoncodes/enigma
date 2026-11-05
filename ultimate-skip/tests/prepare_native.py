#!/usr/bin/env python3
"""Prepare an isolated integration script for the game's native Lua DLL."""
import importlib.util
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("integration_tests", ROOT / "tests/run_tests.py")
tests = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tests)

destination = Path(sys.argv[1]).resolve()
destination.mkdir(parents=True, exist_ok=True)
original = tests.DEFAULT_ORIGINAL.read_bytes()
candidate = tests.CANDIDATE.read_bytes()
original_index = tests.index(original)
candidate_index = tests.index(candidate)
names = [
    ("FightDataHelper", "modules/logic/fight/model/datahelper/FightDataHelper.lua"),
    ("FightCardDataHelper", "modules/logic/fight/model/datahelper/FightCardDataHelper.lua"),
    ("FightCalculateDataMgr", "modules/logic/fight/model/data/FightCalculateDataMgr.lua"),
    ("FightEffectBase", "modules/logic/fight/system/work/FightEffectBase.lua"),
    ("FightWorkEffectDamage", "modules/logic/fight/system/work/FightWorkEffectDamage.lua"),
    ("FightWorkEffectHeal", "modules/logic/fight/system/work/FightWorkEffectHeal.lua"),
    ("FightWorkExPointChange", "modules/logic/fight/system/work/FightWorkExPointChange.lua"),
    ("FightWorkEffectShield", "modules/logic/fight/system/work/FightWorkEffectShield.lua"),
    ("FightWorkEffectShieldChange", "modules/logic/fight/system/work/FightWorkEffectShieldChange.lua"),
    ("FightWorkEffectShieldDel", "modules/logic/fight/system/work/FightWorkEffectShieldDel.lua"),
    ("FightStepEffectFlow", "modules/logic/fight/system/work/FightStepEffectFlow.lua"),
    ("FightWorkStepBuff", "modules/logic/fight/system/work/FightWorkStepBuff.lua"),
    ("FightWorkBuffAddContainer", "modules/logic/fight/system/work/FightWorkBuffAddContainer.lua"),
    ("FightWorkDeadContainer", "modules/logic/fight/system/work/FightWorkDeadContainer.lua"),
    ("FightWorkEffectDeadNew", "modules/logic/fight/system/work/FightWorkEffectDeadNew.lua"),
    ("FightWorkEffectDeadPerformance", "modules/logic/fight/system/work/FightWorkEffectDeadPerformance.lua"),
    ("FightWorkSkillFinallyHeal", "modules/logic/fight/system/work/FightWorkSkillFinallyHeal.lua"),
    ("FightStepEffectWork", "modules/logic/fight/system/work/FightStepEffectWork.lua"),
    ("FightWorkFunction", "modules/logic/fight/system/work/FightWorkFunction.lua"),
    ("FightWorkDelayTimer", "modules/logic/fight/system/work/FightWorkDelayTimer.lua"),
    ("FightWorkTimelineItem", "modules/logic/fight/system/work/FightWorkTimelineItem.lua"),
    ("FightStepBuilder", "modules/logic/fight/system/flow/FightStepBuilder.lua"),
]

chunks = [(ROOT / "tests/harness.lua").read_bytes(), b"\n"]
for name, path in names:
    raw = tests.module_bytes(original, original_index, path)
    chunks.extend([name.encode(), b"=assert(loadstring(", tests.builder.lua_string(raw), b"))()\n"])
raw_candidate = tests.module_bytes(candidate, candidate_index, tests.builder.MODULE_PATH)
raw_original = tests.module_bytes(original, original_index, tests.builder.MODULE_PATH)
settings_original = tests.module_bytes(original, original_index, tests.builder.SETTINGS_PATH)
settings_candidate = tests.module_bytes(candidate, candidate_index, tests.builder.SETTINGS_PATH)
timeline_original = tests.module_bytes(original, original_index, "modules/logic/fight/system/work/FightWorkTimelineItem.lua")
(destination / "integration_setup.lua").write_bytes(b"".join(chunks))
chunks = [(ROOT / "tests/behavior.lua").read_bytes(), b"\n"]
for name in ["test_damage_heal_once", "test_lethal_damage_before_finish", "test_buff_and_damage_once",
             "test_heal_crit_buff_removal_and_shield_absorption",
             "test_resource_and_shield_changes_once",
             "test_death_not_ready_preserves_original", "test_unhandled_paths_use_original", "test_two_modes_and_legacy_preferences",
             "test_ordinary_skills_and_additional_timelines", "test_reported_ultimate_variants_and_device_slots",
             "test_camera_preparation_and_fallback"]:
    chunks.append(name.encode() + b"()\n")
(destination / "integration_checks.lua").write_bytes(b"".join(chunks))
(destination / "original.luao").write_bytes(raw_original)
(destination / "candidate.luao").write_bytes(raw_candidate)
(destination / "settings_original.luao").write_bytes(settings_original)
(destination / "settings_candidate.luao").write_bytes(settings_candidate)
(destination / "timeline_original.luao").write_bytes(timeline_original)
boot_bundle = tests.BOOT_BUNDLE.read_bytes()
boot_index = tests.index(boot_bundle)
for output, module in [("boot_original.luao", "booter/BootStarter.lua"),
                       ("oop_original.luao", "booter/base/oop.lua")]:
    (destination / output).write_bytes(tests.module_bytes(boot_bundle, boot_index, module))
shutil.copy2(ROOT / "tests/bootstrap_setup.lua", destination / "bootstrap_setup.lua")
dll = tests.DEFAULT_ORIGINAL.parents[3] / "Plugins/x86_64/tolua.dll"
shutil.copy2(dll, destination / "tolua.dll")
print(f"Prepared native integration inputs in {destination}")
