-- Run after the game's BootStarter and oop, with original modules in preload.
-- Exercise the first normal skill under the real global guard and object
-- lifecycle; a module-load exception must not be hidden by a simulated class.
local function register(name, path)
    setNeedLoadModule(path:gsub("/", "."):gsub("%.lua$", ""), name)
end
for _, name in ipairs({"FightObject", "FightBaseClass"}) do
    register(name, "modules/logic/fight/" .. name .. ".lua")
end
register("FightWorkItem", "modules/logic/fight/system/work/FightWorkItem.lua")
register("FightWorkTimelineItem", "modules/logic/fight/system/work/FightWorkTimelineItem.lua")
register("FightSkillComp", "modules/logic/fight/entity/comp/FightSkillComp.lua")
register("FightWorkComponent", "modules/logic/fight/fightcomponent/FightWorkComponent.lua")
rawset(_G, "FightWorkItem", nil)

local messages, events = {}, {}
setGlobal("FightEnum", {ActType = {SKILL = 1}, EffectType={}})
setGlobal("FightCardDataHelper", {isBigSkill = function(id) return id == 7777 end})
setGlobal("ResUrl", {getSkillTimeline = function(name) return name end})
setGlobal("Time", {time = 12})
setGlobal("FightHelper", {setBossSkillSpeed = function() end, setTimelineExclusiveSpeed = function() end,
    logForPCSkillEditor=function() end, processSkinByStepData=function(_,mo) return mo.skin end,
    detectReplaceTimeline=function(timeline) return timeline .. '_skin_variant' end,
    isPlayerCardSkill=function(step) return step.cardIndex == 1 end})
setGlobal("FightConfig", {instance={getSkinSkillTimeline=function() return 'base_timeline' end}})
setGlobal("lua_skill", {configDict={[42]={},[7777]={}}})
setGlobal("FightStepBuilder", {ActEffectWorkCls={}})
setGlobal("FightDataHelper", {stateMgr={isReplay=false}, initFightData=function() end})
setGlobal("FightModel", {instance = {updateRTPCSpeed = function() end}})
setGlobal("FightMsgId", {BeforePlayTimeline = "before", PlayTimelineSkill = "play"})
setGlobal("FightMsgMgr", {sendMsg = function(id) table.insert(messages, id) end})
setGlobal("FightEvent", {OnSkillPlayStart = "start"})
setGlobal("FightController", {instance = {dispatchEvent = function(_, id) table.insert(events, id) end}})

local timeline = class("SimulatedUnityTimeline", FightBaseClass)
function timeline:onConstructor(work) self.work = work end
function timeline:play() self.work.originalPlayback = true end
setGlobal("FightSkillTimelineItem", timeline)

local cls = FightSkillComp -- Real BootStarter.__index -> require().
assert(type(cls) == "table" and cls._ultimateSkipPrototypeInstalled)
assert(type(rawget(_G, "UltimateSkipPrototype")) == "table")
assert(#bootstrapErrors == 0, table.concat(bootstrapErrors, "\n"))
assert(package.loaded[getModulePath("FightSkillComp")] == cls,
    "require must cache the class, not an interrupted module environment")
assert(getModuleDef("FightSkillComp") == cls)

local actor = {id = "hero"}
local mo = {id=actor.id, modelId=3003, skin=300301, isCharacter=function() return true end}
function actor:getMO() return mo end
function actor:isMySide() return true end
actor.skill = cls.New(actor)
local step = {actId = 42, actType = 1, fromId = actor.id, actEffect = {}, cardIndex=1}
local work = actor.skill:registPlaySkillWork(42, step)
assert(work.entity == actor and work.fightStepData == step and type(work.CALLBACK) == "table")
assert(rawget(work, "startTimeline") == nil, "normal skill playback must remain inherited")
function work:beforePlayTimeline() self.beforePlayback = true end
work:startTimeline()
assert(work.beforePlayback and work.originalPlayback and work.timelineItem)
assert(#messages == 2 and messages[1] == "before" and messages[2] == "play")
assert(#events == 1 and events[1] == "start")
UltimateSkipPrototype.setEnabled(true)
local ultimate = {actId=7777, actType=1, fromId=actor.id, actEffect={}, cardIndex=1}
local main = actor.skill:registPlaySkillWork(7777, ultimate)
assert(main.timelineName == 'base_timeline_skin_variant' and main._ultimateSkipOriginalStart,
    "the first ultimate must skip even when the game selects a skin variant")
local extra = actor.skill:registTimelineWork('base_timeline_skin_variant', ultimate)
assert(rawget(extra, 'startTimeline') == nil, "direct extra timelines must retain original playback")
assert(#bootstrapErrors == 0, table.concat(bootstrapErrors, "\n"))
