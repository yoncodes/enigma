-- Test scaffolding for actual LuaJIT client modules extracted in memory from
-- the user's bundle. Unity objects, time and the work scheduler are simulated.

function class(name, base)
    local cls = { __cname = name, super = base }
    cls.__index = cls
    setmetatable(cls, { __index = base })
    function cls.New(...)
        local obj = setmetatable({}, cls)
        if obj.onConstructor then obj:onConstructor(...) end
        if obj.onLogicEnter then obj:onLogicEnter(...) end
        if obj.ctor then obj:ctor(...) end
        return obj
    end
    return cls
end

FightBaseClass = class("FightBaseClass")
BaseWork = class("BaseWork")
function BaseWork:start(context)
    self.context = context
    if self.beforeStart then self:beforeStart() end
    return self:onStart(context)
end
function BaseWork:onDone(success)
    assert(not self.WORK_IS_FINISHED, "work completed twice")
    self.WORK_IS_FINISHED = true
    if self._testCompletion then self._testCompletion(success) end
end
function BaseWork:disposeSelf() self.IS_DISPOSED = true end

FightWorkItem = class("FightWorkItem", BaseWork)
FightWorkItem.IS_FIGHT_WORK = true
function FightWorkItem:onConstructor() self.CALLBACK = {} end
function FightWorkItem:onLogicEnter() end
function FightWorkItem:com_sendFightEvent(event, ...)
    FightController.instance:dispatchEvent(event, ...)
end
function FightWorkItem:cancelFightWorkSafeTimer() end
function FightWorkItem:com_registFightEvent(event, callback)
    FightController.instance:registerCallback(event, callback, self)
end
function isTypeOf() return false end

function FightDataClass(name, base) return class(name, base) end
FightDataMgr = { __cname = "FightDataMgr" }
FightLocalDataMgr = { __cname = "FightLocalDataMgr" }
FightDataMgr.instance = { mgrList = {}, initDataMgr = function() end }
FightLocalDataMgr.instance = { mgrList = {}, initDataMgr = function() end }
__G__TRACKBACK__ = function(message) error(message) end
function logError(message) error(message) end
Time = { time = 0 }
local testPrefs = {}
PlayerPrefsHelper = {
    getNumber = function(key, fallback) return testPrefs[key] == nil and fallback or testPrefs[key] end,
    setNumber = function(key, value) testPrefs[key] = value end,
    save = function() end,
}

local effectTypes = {
    MISS = 1, DAMAGE = 2, CRIT = 3, HEAL = 4, BUFFADD = 5,
    BUFFDEL = 6, BUFFUPDATE = 7, DEAD = 9, SHIELD = 25,
    SHIELDCHANGE = 41, BUFFDELNOEFFECT = 56, HEALCRIT = 57,
    SHIELDDEL = 62, KILL = 110, EXPOINTCHANGE = 111,
    EXSKILLPOINTCHANGE = 113, FIGHTSTEP = 162,
    EZIOBIGSKILLDAMAGE = 1000,
}
local unknownEffect = 10000
setmetatable(effectTypes, { __index = function(t, name)
    unknownEffect = unknownEffect + 1
    rawset(t, name, unknownEffect)
    return unknownEffect
end })
FightEnum = {
    ActType = { SKILL = 1, DEVICE = 6 }, EffectType = effectTypes,
    EnchantedType = { Lorenz = 1 },
    EntitySide = { MySide = 1 },
    FloatType = { damage = 8, crit_damage = 4, heal = 7, crit_heal = 3 },
    BehaviourId = { FakeHpToHeal = -1 },
}
FightEvent = setmetatable({}, { __index = function(t, name) rawset(t, name, name); return name end })
FightMsgId = setmetatable({}, { __index = function(t, name) rawset(t, name, name); return name end })

events = {}
listeners = {}
FightController = { instance = {} }
function FightController.instance:registerCallback(event, callback, handle)
    listeners[event] = listeners[event] or {}
    table.insert(listeners[event], {callback = callback, handle = handle})
end
function FightController.instance:unregisterCallback(event, callback, handle)
    local list = listeners[event] or {}
    for i = #list, 1, -1 do
        if list[i].callback == callback and list[i].handle == handle then table.remove(list, i) end
    end
end
function FightController.instance:dispatchEvent(event, ...)
    table.insert(events, event)
    local copy = {}
    for _, item in ipairs(listeners[event] or {}) do table.insert(copy, item) end
    for _, item in ipairs(copy) do item.callback(item.handle, ...) end
end
FightMsgMgr = { sendMsg = function(...) end }
FightSkillBehaviorMgr = { instance = { playSkillEffectBehavior = function(...) end } }
FightGameMgr = { triggerBuffMgr = { triggerBuffEffect = function(...) end } }
FightBuffHelper = { canPlayDormantBuffAni = function() return 0 end }
lua_skill_buff = { configDict = { [501] = {id = 501, typeId = 1, isNoShow = 1} } }
lua_skill_bufftype = { configDict = { [1] = {skipDelay = 1} } }
lua_fight_stacked_buff_combine = { configDict = {} }
FightFloatMgr = { instance = { values = {} } }
function FightFloatMgr.instance:float(id, kind, amount)
    table.insert(self.values, {id = id, kind = kind, amount = amount})
end
FightModel = { instance = { getSpeed = function() return 1 end, getVersion = function() return 1 end,
    updateRTPCSpeed = function() end } }
TaskDispatcher = { runDelay = function(...) end, cancelTask = function(...) end }
tabletool = {
    removeValue = function(list, value)
        for i = #list, 1, -1 do if list[i] == value then table.remove(list, i) end end
    end,
}
function string.nilorempty(value) return value == nil or value == "" end
function string.split(value, separator)
    local result = {}
    local start = 1
    while true do
        local first, last = string.find(value, separator, start, true)
        if not first then table.insert(result, string.sub(value, start)); break end
        table.insert(result, string.sub(value, start, first - 1))
        start = last + 1
    end
    return result
end

entities = {}
FightHelper = {
    getEntity = function(id) return entities[id] end,
    getActEffectData = function() return nil end,
    isPlayerCardSkill = function(step) return step.cardIndex and step.cardIndex > 0 end,
    isASFDSkill = function(id) return id == 8888 end,
    isTimelineStep = function(step) return step.isTimeline == true end,
    logForPCSkillEditor = function() end,
    processSkinByStepData = function(step, mo) return mo.skin end,
    detectReplaceTimeline = function(timeline) return timeline end,
    deepCopySimpleWithMeta = function(value) return value end,
    isRestrain = function() return false end,
    isOppositeByEntityId = function() return true end,
    setBossSkillSpeed = function() end,
    setTimelineExclusiveSpeed = function() end,
    cancelBossSkillSpeed = function() end,
    cancelExclusiveSpeed = function() end,
}
FightConfig = { instance = { getSkinSkillTimeline = function() return "test_timeline" end } }
lua_skill = { configDict = { [7777] = {isBigSkill = 1}, [8888] = {isBigSkill = 1}, [42] = {isBigSkill = 0} } }
ResUrl = { getSkillTimeline = function(name) return name end }
CameraMgr = { instance = { getCameraShake = function()
    return { StopShake = function() end }
end } }
FightSkillMgr = { instance = { isPlayingAnyTimeline = function() return false end } }
FightStepData = {StepType = {Wrap = 1, Inner = 2}}

local timerQueue = {}
function runTimers()
    local current = timerQueue
    timerQueue = {}
    for _, item in ipairs(current) do item.callback(item.handle) end
end
function FightWorkItem:com_registTimer(callback, delay)
    table.insert(timerQueue, {callback = callback, handle = self, delay = delay})
end

FightWorkFlowSequence = class("FightWorkFlowSequence")
function FightWorkFlowSequence:onConstructor() self.works = {} end
function FightWorkFlowSequence:addWork(work) table.insert(self.works, work) end
function FightWorkFlowSequence:registWork(cls, ...)
    assert(cls, "missing effect handler")
    local work = cls.New(...)
    self:addWork(work)
    return work
end
function FightWorkFlowSequence:registFinishCallback(callback, handle)
    self.callback, self.handle = callback, handle
end
function FightWorkFlowSequence:start()
    local function run(index)
        local work = self.works[index]
        if not work then
            if self.callback then self.callback(self.handle, nil, true) end
            if self._testCompletion then self._testCompletion(true) end
            return
        end
        work._testCompletion = function() run(index + 1) end
        work:start()
    end
    run(1)
end
function FightWorkFlowSequence:disposeSelf() self.IS_DISPOSED = true end
function FightWorkItem:com_registFlowSequence() return FightWorkFlowSequence.New() end
function FightWorkItem:com_registWorkDoneFlowSequence()
    local flow = FightWorkFlowSequence.New()
    self._testFlow = flow
    flow:registFinishCallback(function(work) work:onDone(true) end, self)
    return flow
end
function FightWorkItem:com_registWorkDoneFlowParallel()
    local flow = FightWorkFlowSequence.New()
    flow:registFinishCallback(function(work) work:onDone(true) end, self)
    return flow
end
function FightWorkItem:playWorkAndDone() self:onDone(true) end

function makeEntity(id, hp, mySide, modelId)
    local mo = { id = id, modelId = modelId or 3003, skin = 300301,
        currentHp = hp, toughnessValue = 0, toughnessPoint = 0, buffDic = {}, shieldValue = 0, exPoint = 0 }
    function mo:setHp(value) self.currentHp = value end
    function mo:setShield(value) self.shieldValue = value end
    function mo:setDead() self.deadStatus = true end
    function mo:getExPoint() return self.exPoint end
    function mo:setExPoint(value) self.exPoint = value end
    function mo:isCharacter() return true end
    function mo:addBuff(buff) self.buffDic[buff.uid] = buff end
    function mo:delBuff(uid) self.buffDic[uid] = nil end
    function mo:getBuffMO(uid) return self.buffDic[uid] end
    local ui = { hp = hp, _curShield = 0 }
    function ui:getHp() return self.hp end
    function ui:addHp(value) self.hp = self.hp + value end
    function ui:setShield(value) self._curShield = value end
    function ui:setActive(value) self.active = value end
    local entity = { id = id, nameUI = ui, mo = mo, buff = { added = 0, removed = 0 } }
    function entity.buff:addBuff() self.added = self.added + 1 end
    function entity.buff:delBuff() self.removed = self.removed + 1 end
    function entity:getMO() return self.mo end
    function entity:isMySide() return mySide end
    entities[id] = entity
    return entity
end

cjson = { decode = function(text)
    assert(text == "shield-80")
    return { reduceShieldBuffMap = "shield-1#80" }
end }

function resetState()
    events, listeners, entities, timerQueue = {}, {}, {}, {}
    FightFloatMgr.instance.values = {}
    FightDataHelper.stateMgr = { isReplay = false }
    FightDataHelper.initFightData = function() end
    FightDataHelper.roundMgr = { getRoundData = function() return nil end }
    FightDataHelper.tempMgr = { buffDurationDic = {} }
    local cal = FightCalculateDataMgr.New()
    cal.dataMgr = { __cname = "FightDataMgr" }
    function cal.dataMgr:getEntityById(id) return entities[id] and entities[id].mo end
    cal.dataMgr.entityMgr = { addDeadUid = function() end }
    FightDataHelper.calMgr = cal
    FightDataHelper.entityMgr = { getById = function(_, id) return entities[id] and entities[id].mo end }
end

function effect(kind, id, amount)
    local value = { effectType = kind, targetId = id, effectNum = amount, effectNum1 = 0,
        configEffect = 0 }
    function value:isDone() return self.CUSTOM_ISDONE end
    function value:setDone() self.CUSTOM_ISDONE = true end
    if kind == FightEnum.EffectType.DAMAGE or kind == FightEnum.EffectType.CRIT then
        value.hurtInfo = { damage = amount, reduceHp = -amount, reduceShield = 0,
            reduceTeamShareShield = 0, toughnessValue = 0, toughnessPoint = 0,
            absorbHurtParam = "", careerRestraint = false }
    end
    return value
end

function makeComponent(actor)
    local comp = setmetatable({ entity = actor }, FightSkillComp)
    actor.skill = comp
    FightWorkTimelineItem.startTimeline = function(work) work.originalPlayed = true end
    comp.workComp = { registWork = function(_, cls, entity, timeline, step)
        local work = cls.New(entity, timeline, step)
        -- The test exercises the game's real finish function, while replacing
        -- Unity scene setup/cleanup with observable stand-ins.
        function work:beforePlayTimeline() table.insert(events, "before-timeline") end
        function work:afterPlayTimeline() table.insert(events, "after-timeline") end
        return work
    end }
    return comp
end
