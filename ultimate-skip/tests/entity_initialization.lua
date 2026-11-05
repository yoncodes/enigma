-- Use the game's FightObject/FightBaseClass lifecycle and entity initialization;
-- only Unity scene objects and unrelated components are mocked.
function class(name, base)
    local cls = {__cname=name, super=base}
    cls.__index = cls
    setmetatable(cls, {__index=base})
    function cls.New(...)
        local obj = setmetatable({class=cls}, cls)
        if obj.ctor then obj:ctor(...) end
        return obj
    end
    return cls
end
__G__TRACKBACK__ = function(e) error(e) end
function logError(e) error(e) end
gohelper = {create3d=function() return {} end}
SceneTag = {Untagged="Untagged"}
FightEnum = {EntityCreateStage={None=0}}
FightRenderOrderMgr = {instance={register=function() end}}
BossRushController = {instance={isInBossRushInfiniteFight=function() return false end}}
local function dummy(name)
    return class(name, FightBaseClass)
end
function makeOtherComponents()
    FightEffectComp=dummy("FightEffectComp")
    FightBuffComp=dummy("FightBuffComp")
    FightEntityMoveComp=dummy("FightEntityMoveComp")
    FightSkinSpineAction=dummy("FightSkinSpineAction")
    FightSkinSpineEffect=dummy("FightSkinSpineEffect")
    FightTotalDamageComp=dummy("FightTotalDamageComp")
    FightUniqueEffectComp=dummy("FightUniqueEffectComp")
    FightSkinCustomComp=dummy("FightSkinCustomComp")
    FightSpineScaleComp=dummy("FightSpineScaleComp")
    FightNameUI=dummy("FightNameUI")
    TestSpine=dummy("TestSpine")
    TestSpineRenderer=dummy("TestSpineRenderer")
end

function test_entity_initialization()
    FightEntityObject.initHangPointDict=function() end
    FightEntityObject.registClasses=function() end
    FightEntityObject.getSpineClass=function() return TestSpine end
    FightEntityObject.getSpineRendererClass=function() return TestSpineRenderer end
    local root = FightBaseClass.New()
    root._containerGO = {}
    local data = {id="hero1", modelId=3003, skin=300301,
        isAssistBoss=function() return false end, isAct191Boss=function() return false end}
    local entity = root:newClass(FightEntityObject, "hero", data)
    assert(entity.skill and entity.skill.entity == entity, "skill component failed to initialize")
    assert(entity.skill.workComp and entity.skill.workComp.workList, "skill work component missing")
    assert(entity.spine and entity.spineRenderer and entity.nameUI, "entity components missing")
end
