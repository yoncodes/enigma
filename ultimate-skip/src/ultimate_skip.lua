-- Experimental client-side hook. The launcher supplies a default mode and the
-- in-game setting persists later changes. Full battle testing remains necessary.
-- Wrap only the primary skill registration call; entity constructors and
-- supplementary timeline registration remain the original game functions.

local Prototype = {
    enabled = false,
    version = "0.8.0",
}
local MODE_KEY = "CodexUltimateSkipModeV1"
local traceEnabled = os and os.getenv and os.getenv("R1999_ULTIMATE_FIGHT_TRACE") == "1"
local traceWriter
local traceInit = false

local function trace(message)
    if not traceEnabled then return end
    if not traceInit then
        traceInit = true
        local ok, writer = pcall(function()
            local ffi = require("ffi")
            pcall(ffi.cdef, "void ultimate_hook_log(const char *message);")
            local lib = ffi.load("ultimate-skip-hook.dll")
            return function(s) lib.ultimate_hook_log(s .. "\r\n") end
        end)
        if ok then traceWriter = writer end
    end
    if traceWriter then pcall(traceWriter, message) end
end

local function ensureRoundTrace()
    if not traceEnabled or not FightPlayMgr or FightPlayMgr._ultimateSkipTraceInstalled
        or type(FightPlayMgr.playShow) ~= "function" then return end
    local original = FightPlayMgr.playShow
    FightPlayMgr.playShow = function(self, ...)
        -- Diagnostic only: leave the game's choice and return values intact.
        pcall(function()
            trace("BATTLE_ROUND game_skip_performance=" .. tostring(FightGameMgr and FightGameMgr.skipPerformance)
                .. " mode=" .. tostring(Prototype.getMode()))
        end)
        return original(self, ...)
    end
    FightPlayMgr._ultimateSkipTraceInstalled = true
    trace("BATTLE_TRACE round observer installed")
end

local function configuredDefaultMode()
    local value = 0
    if os and os.getenv then
        value = tonumber(os.getenv("R1999_ULTIMATE_SKIP_DEFAULT_MODE")) or 0
    end
    return (value == 0 or value == 2) and value or 0
end

function Prototype.getMode()
    if Prototype.modeOverride ~= nil then
        return Prototype.modeOverride == 2 and 2 or 0
    end
    if not PlayerPrefsHelper then
        return 0
    end
    local mode = PlayerPrefsHelper.getNumber(MODE_KEY, configuredDefaultMode())
    if mode == 0 or mode == 2 then
        return mode
    end
    return 0
end

local function normalSkillStep(step)
    return step and not step.isFakeStep and not step.editorPlaySkill
        and (step.actType == FightEnum.ActType.SKILL or step.actType == FightEnum.ActType.DEVICE)
end

local function isDeviceStep(step)
    if step.actType == FightEnum.ActType.DEVICE then return true end
    local types = FightStepData and FightStepData.StepType
    return types and step.stepType == types.Inner
end


local function shouldSkipForMode(component, skillId, step)
    if not normalSkillStep(step) or skillId ~= step.actId or not component.entity
        or step.fromId ~= component.entity.id or not component.entity:isMySide()
        or FightDataHelper.stateMgr and FightDataHelper.stateMgr.isReplay
        or not (FightHelper.isPlayerCardSkill(step) or isDeviceStep(step))
        or not FightCardDataHelper.isBigSkill(skillId) then
        return false, "not a player ultimate"
    end
    local mode = Prototype.getMode()
    if mode == 0 then return false, "always play mode" end
    return mode == 2, "selected ultimate"
end

local EARLY_EFFECT_NAMES = {
    -- These normally execute during timeline frames. Apply them before the
    -- finish/death notifications. The game's original post-skill flow still
    -- owns buffs, energy, nested actions and all other effects, in its order.
    "DAMAGE", "CRIT", "HEAL", "HEALCRIT", "SHIELD", "SHIELDCHANGE", "SHIELDDEL",
}

local function earlyEffectTypes()
    local result = {}
    for _, name in ipairs(EARLY_EFFECT_NAMES) do
        local value = FightEnum.EffectType[name]
        if value ~= nil then result[value] = true end
    end
    return result
end


local function canAttempt(component, skillId, step)
    if not normalSkillStep(step) or skillId ~= step.actId
        or step.fromId ~= component.entity.id then
        return false, "not the acting character's skill"
    end
    local mo = component.entity:getMO()
    if not mo or not mo.modelId or type(mo.isCharacter) ~= "function" or not mo:isCharacter() then
        return false, "not a character"
    end
    if mo.modelId == 3123 then return false, "interactive Ezio ultimate" end
    -- supportHeroId carries a device slot for DEVICE/Inner steps, not an
    -- assisting hero. Do not mistake The Twins' device selection for support.
    if step.supportHeroId and step.supportHeroId ~= 0 and not isDeviceStep(step) then
        return false, "support character"
    end
    if FightHelper.isASFDSkill and FightHelper.isASFDSkill(skillId) then
        return false, "special skill flow"
    end
    if type(step.actEffect) ~= "table" then return false, "missing effect list" end
    local early = earlyEffectTypes()
    for _, effect in ipairs(step.actEffect) do
        if early[effect.effectType] then
            if effect.fightStep or type(effect.isDone) ~= "function"
                or not FightStepBuilder.ActEffectWorkCls[effect.effectType] then
                return false, "missing early effect handler"
            end
            if (effect.effectType == FightEnum.EffectType.DAMAGE
                or effect.effectType == FightEnum.EffectType.CRIT) and not effect.hurtInfo then
                return false, "missing damage details"
            end
        end
    end
    return true, "primary skill entry"
end

local function buildEarlyWorks(step)
    local result, early = {}, earlyEffectTypes()
    for _, effect in ipairs(step.actEffect) do
        if early[effect.effectType] and not effect:isDone() then
            local cls = FightStepBuilder.ActEffectWorkCls[effect.effectType]
            result[#result + 1] = cls.New(step, effect)
        end
    end
    return result
end

local function deathListenersReady(step)
    for _, effect in ipairs(step.actEffect) do
        -- FightSkillFlow starts its death work in parallel with the timeline.
        -- The effect is marked done before that work starts waiting for the
        -- timeline-finish event. Never finish early and strand the listener.
        if effect.effectType == FightEnum.EffectType.DEAD
            and (not effect.isDone or not effect:isDone()) then
            return false
        end
    end
    return true
end

local function notifyCompletedHeals(work)
    local types = FightEnum.EffectType
    for _, effect in ipairs(work.fightStepData.actEffect) do
        if (effect.effectType == types.HEAL or effect.effectType == types.HEALCRIT)
            and effect.isDone and effect:isDone() then
            -- Prevent FightWorkSkillFinallyHeal from displaying the same heal
            -- a second time when OnSkillPlayFinish is sent.
            FightController.instance:dispatchEvent(FightEvent.OnTimelineHeal, effect)
        end
    end
end

local function effectsFinished(work, _, succeeded)
    if work.IS_DISPOSED or work.WORK_IS_FINISHED then
        return
    end
    if succeeded == false and logError then
        logError("UltimateSkipPrototype: effect flow failed; finishing through the game cleanup path")
    end
    notifyCompletedHeals(work)
    work:onTimelineFinish()
end

local function playOriginal(work)
    work.startTimeline = work._ultimateSkipOriginalStart
    if work._ultimateSkipBypassedPreparation then
        -- A normal timeline needs its original camera preparation and delay.
        -- Restore the start method first so preparation cannot re-enter skip.
        work._ultimateSkipBypassedPreparation = nil
        work.onLoadTimelineDone = work._ultimateSkipOriginalLoad
        return work:onLoadTimelineDone()
    end
    return work:startTimeline()
end

local function runWithoutTimeline(work)
    if work.IS_DISPOSED or work.WORK_IS_FINISHED then
        return
    end
    if not deathListenersReady(work.fightStepData) then
        -- No state was changed yet, so the original playback remains safe.
        return playOriginal(work)
    end

    local ok, effects = pcall(buildEarlyWorks, work.fightStepData)
    if not ok or type(effects) ~= "table" then
        if logError then logError("UltimateSkipPrototype: could not build effect flow: " .. tostring(effects)) end
        return playOriginal(work)
    end

    -- Preserve normal start/finish notifications and cleanup. Only the early
    -- hit/heal/shield data is completed here; all other effects retain the
    -- game's original post-skill flow and cannot consume a later action.
    trace("BATTLE_EXEC skill=" .. tostring(work.skillId) .. " action=skip early_effects=" .. #effects)
    work:dealSpeed()
    work._startTime = Time.time
    work:beforePlayTimeline()
    FightMsgMgr.sendMsg(FightMsgId.PlayTimelineSkill, work.entity, work.skillId, work.fightStepData, work.timelineName)
    FightController.instance:dispatchEvent(FightEvent.OnSkillPlayStart, work.entity, work.skillId, work.fightStepData, work.timelineName)

    local flow = work:com_registFlowSequence()
    for _, effectWork in ipairs(effects) do
        flow:addWork(effectWork)
    end
    flow:registFinishCallback(effectsFinished, work)
    flow:start()
end

local function deferredStart(work)
    -- Let parallel death/heal works register before sending completion events.
    work:com_registTimer(runWithoutTimeline, 0.01)
end

local function skipLoadedTimeline(work)
    if work.IS_DISPOSED or work.WORK_IS_FINISHED then return end
    if gohelper.isNil(work.timelineAssetItem) then
        -- Keep the game's missing-resource error and completion handling.
        return work._ultimateSkipOriginalLoad(work)
    end
    -- onLoadTimelineDone applies camera-distance metadata before startTimeline.
    -- The normal timeline track restores that offset when it is destroyed.
    -- A skipped track never exists, so bypass its preparation as well, keeping
    -- the scene/turn/parallel camera untouched instead of forcing a reset.
    work._ultimateSkipBypassedPreparation = true
    return deferredStart(work)
end

function Prototype.setEnabled(enabled)
    -- Compatibility for the test harness; the in-game setting uses preferences.
    Prototype.enabled = enabled == true
    Prototype.modeOverride = Prototype.enabled and 2 or 0
end

function Prototype.install(FightSkillComp)
    if FightSkillComp._ultimateSkipPrototypeInstalled then return Prototype end
    local original = assert(FightSkillComp.registPlaySkillWork, "primary skill registration is missing")

    function FightSkillComp:registPlaySkillWork(skillId, step, ...)
        -- Let the game select skin/animation variants exactly once. Only this
        -- returned main work can be replaced; direct supplementary timelines
        -- never enter this wrapper, even when they share ultimate step data.
        local work = original(self, skillId, step, ...)
        if not work then return work end
        local modeOk, skip, reason = pcall(shouldSkipForMode, self, skillId, step)
        local ok, eligible = true, false
        if not modeOk then reason = tostring(skip) end
        if modeOk and skip then
            ok, eligible, reason = pcall(canAttempt, self, skillId, step)
            if not ok then reason = tostring(eligible) end
        end
        local willSkip = modeOk and skip and ok and eligible and type(work.startTimeline) == "function"
        if willSkip then
            work._ultimateSkipOriginalStart = work.startTimeline
            work.startTimeline = deferredStart
            if type(work.onLoadTimelineDone) == "function" then
                work._ultimateSkipOriginalLoad = work.onLoadTimelineDone
                work.onLoadTimelineDone = skipLoadedTimeline
            end
        end
        if traceEnabled then
            pcall(function()
                local bigOk, big = pcall(FightCardDataHelper.isBigSkill, skillId)
                local mo = self.entity and self.entity:getMO()
                local codes = {}
                for _, effect in ipairs(step and step.actEffect or {}) do
                    codes[#codes + 1] = tostring(effect.effectType)
                end
                trace("BATTLE_TIMELINE skill=" .. tostring(skillId)
                    .. " model=" .. tostring(mo and mo.modelId)
                    .. " name=" .. tostring(work.timelineName)
                    .. " mode=" .. tostring(Prototype.getMode())
                    .. " big=" .. tostring(bigOk and big)
                    .. " entry=primary"
                    .. " selected=" .. tostring(modeOk and skip)
                    .. " eligible=" .. tostring(ok and eligible)
                    .. " action=" .. (willSkip and "skip" or "play")
                    .. " reason=" .. tostring(reason)
                    .. " effects=" .. table.concat(codes, ","))
                ensureRoundTrace()
            end)
        end
        return work
    end

    FightSkillComp._ultimateSkipPrototypeInstalled = true
    pcall(ensureRoundTrace)
    trace("BATTLE_TRACE prototype " .. Prototype.version .. " primary skill hook")
    return Prototype
end

return Prototype
