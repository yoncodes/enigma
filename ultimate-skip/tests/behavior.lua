local function equal(actual, expected, label)
    assert(actual == expected, string.format("%s: got %s, expected %s", label, tostring(actual), tostring(expected)))
end

local function newStep(actor, effects, skillId)
    return { actType = FightEnum.ActType.SKILL, actId = skillId or 7777,
        fromId = actor.id, cardIndex = 1, supportHeroId = 0, actEffect = effects }
end

local function registeredWork(actor, step)
    return makeComponent(actor):registPlaySkillWork(step.actId, step)
end

local function runEffectFlow(step)
    for _, effectWork in ipairs(FightStepBuilder._buildEffectWorks(step)) do effectWork:start() end
end

local function registerHealFallback(step)
    local work = FightWorkSkillFinallyHeal.New(step)
    for _, value in ipairs(step.actEffect) do
        if value.effectType == FightEnum.EffectType.HEAL or value.effectType == FightEnum.EffectType.HEALCRIT then
            work:addActEffectData(value)
        end
    end
    work:onStart()
    return work
end

function test_camera_preparation_and_fallback()
    -- Run the actual TimelineItem preload callback, not just startTimeline:
    -- camera-distance metadata can move the root before startTimeline runs.
    local previousNil, previousZProj, previousTransform = gohelper, ZProj, transformhelper
    local previousDecode, previousSplit = cjson.decode, string.splitToNumber
    local previousCamera = CameraMgr.instance.getVirtualCameraGO
    local previousPreload, previousFlowStart = FightPreloadOneTimelineRefWork, FightWorkFlowSequence.onStart
    local root = {transform = {x = 1.25, y = -0.5, z = 3}}
    local metadataReads, cameraMoves = 0, 0
    gohelper = {isNil = function(value) return value == nil end}
    ZProj = {SkillTimelineAssetHelper = {GeAssetJson = function()
        metadataReads = metadataReads + 1
        return "camera-distance-metadata"
    end}}
    cjson.decode = function(text)
        if text == "camera-distance-metadata" then return {36, {"9,1,-2"}} end
        return previousDecode(text)
    end
    string.splitToNumber = function(text, separator)
        local values = string.split(text, separator)
        for i, value in ipairs(values) do values[i] = tonumber(value) end
        return values
    end
    transformhelper = {setLocalPos = function(transform, x, y, z)
        cameraMoves = cameraMoves + 1
        transform.x, transform.y, transform.z = x, y, z
    end}
    CameraMgr.instance.getVirtualCameraGO = function() return root end
    FightWorkFlowSequence.onStart = FightWorkFlowSequence.start
    FightPreloadOneTimelineRefWork = class("CameraTestPreload", FightWorkItem)
    function FightPreloadOneTimelineRefWork:onConstructor(_, _, owner) self.owner = owner end
    function FightPreloadOneTimelineRefWork:onStart()
        self.owner:setTimelineAssetItem({})
        self:onDone(true)
    end

    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 500, true)
    local enemy = makeEntity("enemy", 1000, false)
    local hit = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
    local work = registeredWork(actor, newStep(actor, {hit}))
    work:start()
    equal(cameraMoves, 0, "skipped ultimate must not move the camera during preload completion")
    equal(metadataReads, 0, "skipped timeline must bypass camera metadata preparation")
    runTimers()
    equal(work.WORK_IS_FINISHED, true, "skipped ultimate finishes after preload")
    equal(enemy.mo.currentHp, 900, "preloaded skipped ultimate applies damage once")
    equal(root.transform.x, 1.25, "preserve the existing scene or parallel camera x")
    equal(root.transform.y, -0.5, "preserve the existing scene or parallel camera y")
    equal(root.transform.z, 3, "preserve the existing scene or parallel camera z")
    equal(cameraMoves, 0, "skip cleanup does not reset an unrelated camera")

    -- Both ordinary skills and always-play ultimates keep the original camera
    -- preparation and delay before their normal animation starts.
    for _, skillId in ipairs({42, 7777}) do
        UltimateSkipPrototype.setEnabled(skillId == 42)
        work = registeredWork(actor, newStep(actor, {}, skillId))
        work:start()
        equal(root.transform.x, 9, "normal playback keeps original camera preparation")
        equal(work.originalPlayed, nil, "normal playback retains the camera delay")
        runTimers()
        equal(work.originalPlayed, true, "normal playback starts after its camera delay")
    end

    -- If a deferred skip must fall back, restore the complete original loaded
    -- callback. Merely calling startTimeline would omit camera preparation.
    for _, failure in ipairs({"death-listener", "effect-builder"}) do
        resetState()
        UltimateSkipPrototype.setEnabled(true)
        root.transform.x, root.transform.y, root.transform.z = 1.25, -0.5, 3
        actor = makeEntity("hero", 500, true)
        enemy = makeEntity("enemy", 1000, false)
        hit = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
        local effects = {hit}
        if failure == "death-listener" then
            effects[#effects + 1] = effect(FightEnum.EffectType.DEAD, enemy.id, 0)
        end
        work = registeredWork(actor, newStep(actor, effects))
        local previousNew, previousError = FightWorkEffectDamage.New, logError
        if failure == "effect-builder" then
            FightWorkEffectDamage.New = function() error("simulated build failure") end
            logError = function() end
        end
        local reads = metadataReads
        work:start()
        equal(root.transform.x, 1.25, "fallback decision must precede any camera move")
        runTimers()
        FightWorkEffectDamage.New, logError = previousNew, previousError
        equal(metadataReads, reads + 1, "fallback runs original camera preparation exactly once")
        equal(root.transform.x, 9, "fallback uses the animation's original camera offset")
        equal(enemy.mo.currentHp, 1000, "fallback leaves effects to the original timeline")
        runTimers()
        equal(work.originalPlayed, true, "fallback starts the original animation without recursion")
        equal(work.startTimeline, work._ultimateSkipOriginalStart, "fallback restores the original start method")
    end

    gohelper, ZProj, transformhelper = previousNil, previousZProj, previousTransform
    cjson.decode, string.splitToNumber = previousDecode, previousSplit
    CameraMgr.instance.getVirtualCameraGO = previousCamera
    FightPreloadOneTimelineRefWork, FightWorkFlowSequence.onStart = previousPreload, previousFlowStart
end

function test_damage_heal_once()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 400, true)
    local enemy = makeEntity("enemy", 1000, false)
    local hit1 = effect(FightEnum.EffectType.DAMAGE, enemy.id, 300)
    local hit2 = effect(FightEnum.EffectType.CRIT, enemy.id, 200)
    local heal = effect(FightEnum.EffectType.HEAL, actor.id, 250)
    local step = newStep(actor, {hit1, hit2, heal})
    local work = registeredWork(actor, step)
    assert(work._ultimateSkipOriginalStart, "eligible ultimate not intercepted")
    work:startTimeline()
    equal(enemy.mo.currentHp, 1000, "no effect before deferred start")
    local healingFallback = registerHealFallback(step)
    runTimers()
    equal(enemy.mo.currentHp, 500, "server effects applied to enemy model")
    equal(enemy.nameUI.hp, 500, "damage shown on enemy health bar")
    equal(actor.mo.currentHp, 650, "heal applied to ally model")
    equal(actor.nameUI.hp, 650, "heal shown on ally health bar")
    equal(#FightFloatMgr.instance.values, 3, "one float per effect")
    equal(#healingFallback._actEffect, 0, "finish fallback no longer owns heal")
    equal(work.WORK_IS_FINISHED, true, "timeline work finishes")
    assert(hit1:isDone() and hit2:isDone() and heal:isDone(), "effects were not marked complete")
    runEffectFlow(step) -- The original FightSkillFlow runs its post-skill flow later.
    equal(enemy.mo.currentHp, 500, "post-skill flow did not repeat damage")
    equal(actor.mo.currentHp, 650, "post-skill flow did not repeat healing")
    equal(actor.nameUI.hp, 650, "post-skill flow did not repeat heal UI")
    equal(#FightFloatMgr.instance.values, 3, "post-skill flow did not repeat floats")
end

function test_lethal_damage_before_finish()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 1000, true)
    local enemy = makeEntity("enemy", 100, false)
    local hit = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
    local dead = effect(FightEnum.EffectType.DEAD, enemy.id, 0)
    local step = newStep(actor, {hit, dead})
    local work = registeredWork(actor, step)
    work:startTimeline()
    -- Start the real parallel death-performance work: it marks the data done,
    -- constructs the real death work and registers its finish listener.
    local deathPerformance = FightWorkEffectDeadPerformance.New(step, dead, true)
    deathPerformance:start()
    assert(dead:isDone() and enemy.mo.deadStatus, "death work did not register")
    local observed = {}
    local deathWork = deathPerformance._testFlow.works[1]
    function deathWork:_playDeadWork()
        observed.hp = enemy.mo.currentHp
        observed.ui = enemy.nameUI.hp
        self:onDone(true)
    end
    runTimers()
    equal(observed.hp, 0, "damage applied before death listener")
    equal(observed.ui, 0, "damage displayed before death listener")
    equal(work.WORK_IS_FINISHED, true, "lethal work finishes")
end

function test_buff_and_damage_once()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 1000, true)
    local enemy = makeEntity("enemy", 1000, false)
    local buff = effect(FightEnum.EffectType.BUFFADD, actor.id, 0)
    buff.buff = {uid = "buff-1", buffId = 501, duration = 2}
    local hit = effect(FightEnum.EffectType.DAMAGE, enemy.id, 200)
    local step = newStep(actor, {buff, hit})
    local work = registeredWork(actor, step)
    assert(work._ultimateSkipOriginalStart, "buff ultimate not intercepted")
    work:startTimeline()
    runTimers()
    equal(actor.buff.added, 0, "buff keeps the original post-skill animation")
    equal(enemy.mo.currentHp, 800, "damage after buff")
    runEffectFlow(step)
    assert(actor.mo.buffDic["buff-1"], "buff not applied to model")
    equal(actor.buff.added, 1, "post-skill flow did not repeat buff")
    equal(enemy.mo.currentHp, 800, "post-skill flow did not repeat damage")
end

function test_heal_crit_buff_removal_and_shield_absorption()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 300, true)
    local enemy = makeEntity("enemy", 1000, false)
    enemy.mo.shieldValue = 200
    enemy.nameUI._curShield = 200
    actor.mo.buffDic["buff-1"] = {uid = "buff-1", buffId = 501, duration = 2}
    local removed = effect(FightEnum.EffectType.BUFFDEL, actor.id, 0)
    removed.buff = {uid = "buff-1", buffId = 501, duration = 2}
    local hit = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
    hit.hurtInfo.reduceHp = -20
    hit.hurtInfo.absorbHurtParam = "shield-80"
    local heal = effect(FightEnum.EffectType.HEALCRIT, actor.id, 150)
    local step = newStep(actor, {removed, hit, heal})
    local work = registeredWork(actor, step)
    assert(work._ultimateSkipOriginalStart, "supported effects not intercepted")
    work:startTimeline()
    registerHealFallback(step)
    runTimers()
    equal(actor.buff.removed, 0, "buff removal stays in the original post-skill flow")
    equal(enemy.mo.currentHp, 980, "shielded damage health")
    equal(enemy.mo.shieldValue, 120, "shielded damage model shield")
    equal(enemy.nameUI._curShield, 120, "shielded damage UI shield")
    equal(actor.mo.currentHp, 450, "critical healing applied once")
    equal(actor.nameUI.hp, 450, "critical healing displayed once")
    runEffectFlow(step)
    equal(actor.mo.buffDic["buff-1"], nil, "buff removed from model")
    equal(actor.buff.removed, 1, "buff removal not repeated")
    equal(enemy.mo.currentHp, 980, "shielded damage not repeated")
    equal(actor.mo.currentHp, 450, "critical heal not repeated")
end

function test_resource_and_shield_changes_once()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 500, true)
    actor.mo.exPoint = 5
    local shield = effect(FightEnum.EffectType.SHIELD, actor.id, 200)
    local conversion = effect(FightEnum.EffectType.SHIELDCHANGE, actor.id, 30)
    local shieldAgain = effect(FightEnum.EffectType.SHIELD, actor.id, 90)
    local remove = effect(FightEnum.EffectType.SHIELDDEL, actor.id, 0)
    local cost = effect(FightEnum.EffectType.EXPOINTCHANGE, actor.id, -5)
    local step = newStep(actor, {shield, conversion, shieldAgain, remove, cost})
    local work = registeredWork(actor, step)
    assert(work._ultimateSkipOriginalStart, "shield and resource ultimate not intercepted")
    work:startTimeline()
    runTimers()
    equal(actor.mo.currentHp, 530, "shield conversion model health")
    equal(actor.nameUI.hp, 530, "shield conversion UI health")
    equal(actor.mo.shieldValue, 0, "shield added and removed in model")
    equal(actor.nameUI._curShield, 0, "shield added and removed in UI")
    equal(actor.mo.exPoint, 5, "resource cost remains in the original post-skill flow")
    equal(#FightFloatMgr.instance.values, 1, "shield conversion float once")
    runEffectFlow(step)
    equal(actor.mo.currentHp, 530, "shield conversion not repeated")
    equal(actor.mo.exPoint, 0, "resource cost not repeated")
    equal(#FightFloatMgr.instance.values, 1, "shield float not repeated")
end

function test_death_not_ready_preserves_original()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 1000, true)
    local enemy = makeEntity("enemy", 100, false)
    local hit = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
    local dead = effect(FightEnum.EffectType.DEAD, enemy.id, 0)
    local work = registeredWork(actor, newStep(actor, {hit, dead}))
    work:startTimeline()
    runTimers()
    equal(work.originalPlayed, true, "unready death listener falls back")
    equal(enemy.mo.currentHp, 100, "fallback has not applied damage early")
end

function test_unhandled_paths_use_original()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 1000, true)
    local enemy = makeEntity("enemy", 1000, false)
    local unknown = effect(99999, enemy.id, 0)
    local work = registeredWork(actor, newStep(actor, {unknown}))
    assert(work._ultimateSkipOriginalStart, "an unknown post-skill effect must not block skipping the main animation")
    work:startTimeline()
    runTimers()
    equal(unknown:isDone(), nil, "unknown effect is left to the game")

    actor.mo.modelId = 3123
    work = registeredWork(actor, newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)}))
    equal(work._ultimateSkipOriginalStart, nil, "Ezio's interaction is not intercepted")
    work:startTimeline()
    equal(work.originalPlayed, true, "Ezio plays normally")

    actor.mo.modelId = 3003
    FightDataHelper.stateMgr.isReplay = true
    work = registeredWork(actor, newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)}))
    equal(work._ultimateSkipOriginalStart, nil, "replay is not intercepted")
    FightDataHelper.stateMgr.isReplay = false

    local nonUltimate = registeredWork(actor, newStep(actor, {}, 42))
    equal(nonUltimate._ultimateSkipOriginalStart, nil, "ordinary skill is not intercepted")

    work = registeredWork(actor, newStep(actor, {}))
    assert(work._ultimateSkipOriginalStart, "a pure animation ultimate can skip without consuming effects")
    work = registeredWork(actor, newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)}, 8888))
    equal(work._ultimateSkipOriginalStart, nil, "special ASFD skill is not intercepted")

    local otherSource = newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)})
    otherSource.fromId = "another-entity"
    work = registeredWork(actor, otherSource)
    equal(work._ultimateSkipOriginalStart, nil, "different skill owner is not intercepted")

    local nested = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
    nested.fightStep = {}
    work = registeredWork(actor, newStep(actor, {nested}))
    equal(work._ultimateSkipOriginalStart, nil, "nested effect is not intercepted")

    local support = newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)})
    support.supportHeroId = 1
    work = registeredWork(actor, support)
    equal(work._ultimateSkipOriginalStart, nil, "support hero is not intercepted")

    local added = effect(FightEnum.EffectType.BUFFADD, actor.id, 0)
    added.buff = {uid = "buff-next-check", buffId = 501, duration = 2}
    local buffStep = newStep(actor, {added})
    buffStep.custom_stepIndex = 1
    work = registeredWork(actor, buffStep)
    assert(work._ultimateSkipOriginalStart, "post-skill buffs need not inspect unavailable later steps")
    local later = { actEffect = {effect(FightEnum.EffectType.BUFFADD, actor.id, 0)} }
    FightDataHelper.roundMgr = { getRoundData = function() return {fightStep = {buffStep, later}} end }
    work = registeredWork(actor, buffStep)
    assert(work._ultimateSkipOriginalStart, "later effect grouping must remain owned by the original post-skill flow")
    work:startTimeline()
    runTimers()
    equal(added:isDone(), nil, "guard did not consume the current buff")
    equal(later.actEffect[1]:isDone(), nil, "guard did not consume the later buff")
    FightDataHelper.roundMgr = { getRoundData = function() return {fightStep = {buffStep}} end }
    work = registeredWork(actor, buffStep)
    assert(work._ultimateSkipOriginalStart, "a final buff step remains eligible")

    local enemyActor = makeEntity("opponent", 1000, false)
    work = registeredWork(enemyActor, newStep(enemyActor, {effect(FightEnum.EffectType.DAMAGE, actor.id, 100)}))
    equal(work._ultimateSkipOriginalStart, nil, "enemy ultimate is not intercepted")

    local builder = FightWorkEffectDamage.New
    work = registeredWork(actor, newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)}))
    FightWorkEffectDamage.New = function() error("simulated build failure") end
    local previousLogError = logError
    logError = function() end
    work:startTimeline()
    runTimers()
    logError = previousLogError
    FightWorkEffectDamage.New = builder
    equal(work.originalPlayed, true, "builder failure falls back before effects start")
    equal(enemy.mo.currentHp, 1000, "builder failure did not apply damage")

    UltimateSkipPrototype.setEnabled(false)
    work = registeredWork(actor, newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)}))
    equal(work._ultimateSkipOriginalStart, nil, "prototype disabled by default")
end

function test_two_modes_and_legacy_preferences()
    resetState()
    UltimateSkipPrototype.modeOverride = nil
    local actor = makeEntity("hero", 1000, true)
    local enemy = makeEntity("enemy", 1000, false)
    local initFightData = FightDataHelper.initFightData
    local function work()
        return registeredWork(actor, newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)}))
    end
    PlayerPrefsHelper.setNumber("CodexUltimateSkipModeV1", 2)
    assert(work()._ultimateSkipOriginalStart, "always skip includes the first ultimate")
    assert(work()._ultimateSkipOriginalStart, "always skip includes every later ultimate")
    FightDataHelper.initFightData({})
    assert(work()._ultimateSkipOriginalStart, "always skip does not depend on battle history")
    equal(FightDataHelper.initFightData, initFightData, "no battle lifecycle wrapper is installed")
    PlayerPrefsHelper.setNumber("CodexUltimateSkipModeV1", 0)
    equal(work()._ultimateSkipOriginalStart, nil, "always play mode")
    PlayerPrefsHelper.setNumber("CodexUltimateSkipModeV1", 1)
    equal(UltimateSkipPrototype.getMode(), 0, "removed saved mode uses always play")
    equal(work()._ultimateSkipOriginalStart, nil, "removed mode cannot activate skipping")
    PlayerPrefsHelper.setNumber("CodexUltimateSkipModeV1", 0)
end

function test_ordinary_skills_and_additional_timelines()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local actor = makeEntity("hero", 1000, true)
    local enemy = makeEntity("enemy", 1000, false)
    local component = makeComponent(actor)
    local originalTimelineLookup = FightConfig.instance.getSkinSkillTimeline
    local lookupCount = 0
    FightConfig.instance.getSkinSkillTimeline = function(...)
        lookupCount = lookupCount + 1
        return originalTimelineLookup(...)
    end
    local directNormal = component:registTimelineWork("ordinary_extra", newStep(actor, {}, 42))
    equal(lookupCount, 0, "ordinary auxiliary timelines need no extra configuration lookup")
    equal(directNormal._ultimateSkipOriginalStart, nil, "ordinary auxiliary timeline is untouched")
    FightConfig.instance.getSkinSkillTimeline = originalTimelineLookup
    local function ordinary(label)
        local damage = effect(FightEnum.EffectType.DAMAGE, enemy.id, 100)
        local energy = effect(FightEnum.EffectType.EXPOINTCHANGE, actor.id, 1)
        local step = newStep(actor, {damage, energy}, 42)
        local work = component:registPlaySkillWork(42, step)
        equal(work._ultimateSkipOriginalStart, nil, label .. " must not be intercepted")
        work:startTimeline()
        equal(work.originalPlayed, true, label .. " must play its original timeline")
        equal(damage:isDone(), nil, label .. " damage must not be applied early")
        equal(energy:isDone(), nil, label .. " energy must not be applied early")
    end
    ordinary("ordinary skill before any ultimate")

    local ultimate = newStep(actor, {effect(FightEnum.EffectType.DAMAGE, enemy.id, 200)})
    local extra = component:registTimelineWork("special_resource_or_buff_timeline", ultimate)
    equal(extra._ultimateSkipOriginalStart, nil, "an ultimate's supplementary timeline must play")
    extra:startTimeline()
    equal(extra.originalPlayed, true, "supplementary timeline plays normally")
    local originalDetect = FightHelper.detectReplaceTimeline
    local detects = 0
    FightHelper.detectReplaceTimeline = function()
        detects = detects + 1
        return "dynamic_random_replacement"
    end
    local replaced = component:registPlaySkillWork(ultimate.actId, ultimate)
    FightHelper.detectReplaceTimeline = originalDetect
    equal(detects, 1, "dynamic timeline selection must not be repeated")
    assert(replaced._ultimateSkipOriginalStart, "a registered ultimate must skip even with a dynamic timeline")
    replaced:startTimeline()
    runTimers()
    equal(enemy.mo.currentHp, 800, "dynamic ultimate applies its damage")
    local main = component:registPlaySkillWork(ultimate.actId, ultimate)
    assert(main._ultimateSkipOriginalStart, "the matching ultimate timeline should be intercepted")
    main:startTimeline()
    runTimers()
    equal(enemy.mo.currentHp, 800, "same-step re-registration must not repeat damage")
    ordinary("ordinary skill after a skipped ultimate")

    UltimateSkipPrototype.modeOverride = nil
    PlayerPrefsHelper.setNumber("CodexUltimateSkipModeV1", 0)
    main = component:registPlaySkillWork(ultimate.actId, ultimate)
    equal(main._ultimateSkipOriginalStart, nil, "always play mode keeps ultimate animation")
    ordinary("ordinary skill in always play mode")
end

function test_reported_ultimate_variants_and_device_slots()
    resetState()
    UltimateSkipPrototype.setEnabled(true)
    local configLookup, detect = FightConfig.instance.getSkinSkillTimeline, FightHelper.detectReplaceTimeline
    local cases = {
        {31495151,3149,"shuangshengwuling_314903_unique_2",true},
        {31495151,3149,"shuangshengwuling_314903_unique_4","inner"},
        {31445131,3144,"falangyan_314403_unique"},
        {302554312,3025,"xingti2hao_302501_unique"},
        {31450135,3145,"huishengyao_314503_unique"},
    }
    for i, data in ipairs(cases) do
        local skillId, modelId, variant, device = unpack(data)
        lua_skill.configDict[skillId] = {isBigSkill=1}
        local actor = makeEntity("actor"..i,1000,true,modelId)
        actor.mo.exPoint = 100
        local target = makeEntity("target"..i,1000,false)
        local component = makeComponent(actor)
        local hit = effect(FightEnum.EffectType.DAMAGE,target.id,200)
        local energy = effect(FightEnum.EffectType.EXPOINTCHANGE,actor.id,-100)
        local marker = effect(99998,actor.id,6)
        local step = newStep(actor,{hit,marker,energy},skillId)
        if device then
            step.supportHeroId=1
            if device == "inner" then step.stepType=FightStepData.StepType.Inner;step.cardIndex=0
            else step.actType=FightEnum.ActType.DEVICE end
        end
        FightConfig.instance.getSkinSkillTimeline = function() return "base_character_unique" end
        local selections=0
        FightHelper.detectReplaceTimeline = function() selections=selections+1;return variant end
        local work = component:registPlaySkillWork(skillId,step)
        equal(selections,1,"variant selection runs exactly once")
        assert(work._ultimateSkipOriginalStart,"reported ultimate variant must skip: "..variant)
        local supplementary = component:registTimelineWork(variant,step)
        equal(supplementary._ultimateSkipOriginalStart,nil,"even the same-named direct extra timeline is untouched")
        work:startTimeline();runTimers()
        equal(target.mo.currentHp,800,"variant damage is applied once")
        equal(actor.mo.exPoint,100,"device energy keeps its normal post-skill flow")
        equal(marker:isDone(),nil,"character-specific markers remain owned by the game")
        runEffectFlow(step)
        equal(target.mo.currentHp,800,"post-skill damage is not repeated")
        equal(actor.mo.exPoint,0,"the original flow applies device energy exactly once")
    end
    FightConfig.instance.getSkinSkillTimeline, FightHelper.detectReplaceTimeline = configLookup, detect
end
