-- Dependencies for running the original BootStarter and oop bytecode. Keep
-- their global-write guard and lazy module loader completely intact.
bootstrapErrors = {}
SLFramework = {
    FrameworkSettings = {Is64Bit = true},
    SLLogger = {CanLogNormal = false, CanLogWarn = false, CanLogError = true},
    UnityHelper = {IsDebugBuild = function() return false end},
    GameConfig = {Instance = {}},
}
printError = function(message) table.insert(bootstrapErrors, message) end
forceLog = function() end
savedBootstrapRequire = require
require = function() return {} end -- Unrelated boot services only.

-- A base is sufficient for the native module-load regression. Python tests
-- replace it with the actual lazy-loaded FightWorkItem and its ancestors.
FightWorkItem = {}
