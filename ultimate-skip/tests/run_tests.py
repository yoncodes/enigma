#!/usr/bin/env python3
"""Exercise the hook with bytecode from the real client and simulated Unity."""

import hashlib
import importlib.util
from pathlib import Path
import unittest

from lupa.luajit21 import LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ORIGINAL = Path(
    "/opt/data/private/1999/enigma-v65-remote-build-20261031/client-headless-20261102/"
    "client-core/reverse1999_Data/StreamingAssets/PersistentRoot/luabytes/"
    "9c2019bedb92e2327bfe12024e2922a4.dat"
)
CANDIDATE = ROOT / "candidate/luabytes-prototype.dat"
BOOT_BUNDLE = DEFAULT_ORIGINAL.parents[2] / "Windows/luabytes/9981b62cca43bcb97f4b9a8f14b534de.dat"

spec = importlib.util.spec_from_file_location("build_candidate", ROOT / "tools/build_candidate.py")
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


def index(bundle):
    return {tag: (start, end) for tag, _, start, end in builder.entries(bundle)}


def module_bytes(bundle, bundle_index, path):
    tag = hashlib.md5(path.encode("utf-8")).hexdigest().encode("ascii")
    start, end = bundle_index[tag]
    return builder.unpack_record(bundle, tag, start, end)


class PrototypeIntegration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.original = DEFAULT_ORIGINAL.read_bytes()
        cls.candidate = CANDIDATE.read_bytes()
        cls.orig_index = index(cls.original)
        cls.new_index = index(cls.candidate)

    def setUp(self):
        self.lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        self.lua.execute((ROOT / "tests/harness.lua").read_bytes())
        load = self.lua.eval(b"function(data) return assert(loadstring(data))() end")
        modules = [
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
        for name, path in modules:
            self.lua.globals()[name.encode()] = load(module_bytes(self.original, self.orig_index, path))
        self.lua.globals()[b"FightSkillComp"] = load(
            module_bytes(self.candidate, self.new_index, builder.MODULE_PATH)
        )
        self.lua.execute((ROOT / "tests/behavior.lua").read_bytes())

    def test_damage_and_healing_are_applied_once(self):
        self.lua.globals().test_damage_heal_once()

    def test_lethal_damage_precedes_death_event(self):
        self.lua.globals().test_lethal_damage_before_finish()

    def test_buff_and_damage_are_applied_once(self):
        self.lua.globals().test_buff_and_damage_once()

    def test_heal_crit_buff_removal_and_shield_absorption(self):
        self.lua.globals().test_heal_crit_buff_removal_and_shield_absorption()

    def test_resource_and_shield_changes_are_applied_once(self):
        self.lua.globals().test_resource_and_shield_changes_once()

    def test_unready_death_listener_falls_back(self):
        self.lua.globals().test_death_not_ready_preserves_original()

    def test_special_and_unknown_paths_fall_back(self):
        self.lua.globals().test_unhandled_paths_use_original()

    def test_two_modes_and_legacy_preferences(self):
        self.lua.globals().test_two_modes_and_legacy_preferences()

    def test_ordinary_skills_and_additional_timelines_play(self):
        self.lua.globals().test_ordinary_skills_and_additional_timelines()

    def test_reported_ultimate_variants_and_device_slots(self):
        self.lua.globals().test_reported_ultimate_variants_and_device_slots()

    def test_camera_preparation_is_bypassed_only_for_skipped_ultimates(self):
        self.lua.globals().test_camera_preparation_and_fallback()


class SettingsIntegration(unittest.TestCase):
    def test_dropdown_and_persistence(self):
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        lua.execute((ROOT / "tests/settings_behavior.lua").read_bytes())
        addon = lua.execute((ROOT / "src/settings_graphics.lua").read_bytes())
        lua.globals().test_settings_addon(addon)

    def test_text_language_fallback_and_live_refresh(self):
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        lua.execute((ROOT / "tests/settings_behavior.lua").read_bytes())
        addon = lua.execute((ROOT / "src/settings_graphics.lua").read_bytes())
        lua.globals().test_settings_localization(addon)


class BattleTrace(unittest.TestCase):
    def test_round_observer_and_normal_timeline_preserve_original(self):
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        lua.execute((ROOT / "tests/harness.lua").read_bytes())
        original = DEFAULT_ORIGINAL.read_bytes()
        load = lua.eval(b"function(data) return assert(loadstring(data))() end")
        lua.globals()[b"FightCardDataHelper"] = load(module_bytes(
            original, index(original), "modules/logic/fight/model/datahelper/FightCardDataHelper.lua"))
        lua.execute(b'''
            captured = {}
            local previousGetenv = os.getenv
            os.getenv = function(key)
                if key == 'R1999_ULTIMATE_FIGHT_TRACE' then return '1' end
                return previousGetenv(key)
            end
            package.loaded.ffi = {
                cdef = function() end,
                load = function() return {ultimate_hook_log = function(s) table.insert(captured, s) end} end,
            }
            FightGameMgr.skipPerformance = true
            FightPlayMgr = {playShow = function(self, a, b) self.called = true; return a, b end}
            FightDataHelper = {stateMgr = {isReplay = false}}
        ''')
        addon = lua.execute((ROOT / "src/ultimate_skip.lua").read_bytes())
        lua.globals()[b"addon"] = addon
        lua.execute(b'''
            local cls = {}
            function cls:registPlaySkillWork()
                return {timelineName='test_timeline', startTimeline=function(self) self.played=true end}
            end
            addon.install(cls)
            addon.setEnabled(true)
            local a, b = FightPlayMgr:playShow('unchanged', 7)
            assert(a == 'unchanged' and b == 7 and FightPlayMgr.called)
            local actor = {id = 'unit'}
            local mo = {id = 'unit', skin = 1, modelId = 2}
            function actor:getMO() return mo end
            function actor:isMySide() return true end
            actor.skill = setmetatable({entity = actor}, {__index=cls})
            local step = {actType = 1, actId = 42, fromId = 'unit', cardIndex = 1, actEffect = {}}
            local work = actor.skill:registPlaySkillWork(42, step)
            work:startTimeline()
            assert(work.played and work._ultimateSkipOriginalStart == nil)
            local log = table.concat(captured)
            assert(log:find('game_skip_performance=true', 1, true))
            assert(log:find('skill=42', 1, true) and log:find('action=play', 1, true))
        ''')


class EntityInitialization(unittest.TestCase):
    def test_original_skill_component_and_entity_lifecycle(self):
        original = DEFAULT_ORIGINAL.read_bytes()
        original_index = index(original)
        candidate = CANDIDATE.read_bytes()
        candidate_index = index(candidate)
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        lua.execute((ROOT / "tests/entity_initialization.lua").read_bytes())
        load = lua.eval(b"function(data) return assert(loadstring(data))() end")
        for name, path in [
            ("FightObject", "modules/logic/fight/FightObject.lua"),
            ("FightBaseClass", "modules/logic/fight/FightBaseClass.lua"),
            ("FightWorkComponent", "modules/logic/fight/fightcomponent/FightWorkComponent.lua"),
            ("FightSkillComp", "modules/logic/fight/entity/comp/FightSkillComp.lua"),
            ("FightEntityObject", "modules/logic/fight/entity/FightEntityObject.lua"),
        ]:
            lua.globals()[name.encode()] = load(module_bytes(original, original_index, path))
        lua.globals().makeOtherComponents()
        lua.globals()[b"FightWorkItem"] = lua.globals()[b"FightBaseClass"]
        before = lua.eval(b"string.dump(FightSkillComp.onConstructor)")
        lua.globals()[b"FightSkillComp"] = load(module_bytes(candidate, candidate_index, builder.MODULE_PATH))
        after = lua.eval(b"string.dump(FightSkillComp.onConstructor)")
        self.assertEqual(before, after)
        lua.globals().test_entity_initialization()


class RealBootstrap(unittest.TestCase):
    def test_lazy_module_load_and_first_normal_skill_with_global_guard(self):
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        lua.execute((ROOT / "tests/bootstrap_setup.lua").read_bytes())
        boot = BOOT_BUNDLE.read_bytes()
        boot_index = index(boot)
        lua.execute(module_bytes(boot, boot_index, "booter/BootStarter.lua"))
        lua.execute(b"require=savedBootstrapRequire")
        lua.execute(module_bytes(boot, boot_index, "booter/base/oop.lua"))
        original = DEFAULT_ORIGINAL.read_bytes()
        original_index = index(original)
        candidate = CANDIDATE.read_bytes()
        compile_module = lua.eval(b"function(data) return assert(loadstring(data)) end")
        for path in [
            "modules/logic/fight/FightObject.lua",
            "modules/logic/fight/FightBaseClass.lua",
            "modules/logic/fight/system/work/FightWorkItem.lua",
            "modules/logic/fight/fightcomponent/FightWorkComponent.lua",
            "modules/logic/fight/system/work/FightWorkTimelineItem.lua",
            builder.MODULE_PATH,
        ]:
            blob = candidate if path == builder.MODULE_PATH else original
            blob_index = index(candidate) if path == builder.MODULE_PATH else original_index
            key = path[:-4].replace("/", ".").encode()
            lua.globals().package.preload[key] = compile_module(module_bytes(blob, blob_index, path))
        lua.execute((ROOT / "tests/real_bootstrap_behavior.lua").read_bytes())


class ModuleWrapper(unittest.TestCase):
    def test_loader_environment_and_arguments_are_preserved(self):
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        make = lua.eval(b"function() return string.dump(assert(loadstring('local arg=...; return {arg=arg, value=only_in_environment}'))) end")
        wrapper = builder.compile_wrapper(make(), b"return {install=function(original) original.installed=true; return {} end}")
        result = lua.eval(b'''function(data)
            local env=setmetatable({only_in_environment=123},{__index=_G})
            local fn=assert(loadstring(data)); setfenv(fn,env)
            local value=fn('module-name')
            return value.arg,value.value,value.installed
        end''')(wrapper)
        self.assertEqual(result, (b"module-name", 123, True))

    def test_failed_addon_keeps_original_module(self):
        lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
        make = lua.eval(b"function() return string.dump(assert(loadstring('return {valid=true}'))) end")
        wrapper = builder.compile_wrapper(make(), b"return {install=function() error('simulated failure') end}")
        result = lua.eval(b"function(data) return assert(loadstring(data))().valid end")(wrapper)
        self.assertTrue(result)


if __name__ == "__main__":
    unittest.main(verbosity=2)
