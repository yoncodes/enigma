-- Adds a separate dropdown by cloning the existing video-mode settings row.
-- Its callback stores the mode in the same local preferences as the game.
local SettingsAddon = {}
local MODE_KEY = "CodexUltimateSkipModeV1"
-- Mode ids stay 0/2 so an existing "always skip" preference is preserved.
-- The dropdown indices are 0/1; unsupported saved values use "always play".
local MODES = { 0, 2 }
local TEXT = {
    zh = {
        title = "至终的仪式动画",
        description = "选择大招动画的播放方式",
        options = { "始终播放", "始终跳过" },
    },
    tw = {
        title = "至終的儀式動畫",
        description = "選擇大招動畫的播放方式",
        options = { "始終播放", "始終跳過" },
    },
    en = {
        title = "Ultimate animation",
        description = "Choose how Ultimate animations are played.",
        options = { "Always play", "Always skip" },
    },
}

local function currentText()
    local ok, language = pcall(function() return GameConfig:GetCurLangShortcut() end)
    if ok and (language == "zh" or language == "tw") then return TEXT[language] end
    return TEXT.en
end

local function defaultMode()
    local value = os and os.getenv and tonumber(os.getenv("R1999_ULTIMATE_SKIP_DEFAULT_MODE")) or 0
    return (value == 0 or value == 2) and value or 0
end

local function currentMode()
    local value = PlayerPrefsHelper.getNumber(MODE_KEY, defaultMode())
    return (value == 0 or value == 2) and value or 0
end

local function setLabels(row, drop, switch)
    local found = {}
    local function collect(kind)
        local ok, texts = pcall(function() return row:GetComponentsInChildren(typeof(kind), true) end)
        if not ok or not texts then return end
        for index = 0, texts.Length - 1 do
            local item = texts[index]
            local tr = item and item.transform
            if tr and not tr:IsChildOf(drop.gameObject.transform)
                and (not switch or not tr:IsChildOf(switch.transform)) then
                table.insert(found, item)
            end
        end
    end
    if TMPro and TMPro.TMP_Text then collect(TMPro.TMP_Text) end
    if #found == 0 and UnityEngine and UnityEngine.UI then collect(UnityEngine.UI.Text) end
    if #found == 0 then error("settings row has no editable label") end
    for _, label in ipairs(found) do
        local ok, lang = pcall(function() return label.gameObject:GetComponent(typeof(SLFramework.LangTxt)) end)
        if ok and lang then UnityEngine.Object.Destroy(lang) end
    end
    return found
end

local function refresh(view)
    local drop = view._ultimateSkipDrop
    if not drop then return end
    local text = currentText()
    local labels = view._ultimateSkipLabels
    if labels and labels[1] then labels[1].text = text.title end
    if labels and labels[2] then labels[2].text = text.description end
    view._ultimateSkipUpdating = true
    drop:ClearOptions()
    drop:AddOptions(text.options)
    if view._ultimateSkipTemplate then
        recthelper.setHeight(view._ultimateSkipTemplate.transform, #text.options * 73)
    end
    drop:SetValue(currentMode() == 2 and 1 or 0)
    view._ultimateSkipUpdating = false
end

local function onValueChanged(view, index)
    if view._ultimateSkipUpdating or (index ~= 0 and index ~= 1) then return end
    PlayerPrefsHelper.setNumber(MODE_KEY, MODES[index + 1])
    PlayerPrefsHelper.save()
    if AudioMgr and AudioMgr.instance and AudioEnum and AudioEnum.UI then
        AudioMgr.instance:trigger(AudioEnum.UI.play_ui_set_select)
    end
end

local function init(view)
    local original = gohelper.findChild(view.viewGO, "graphicsScroll/Viewport/Content/videomode")
    if not original then error("video-mode settings row is missing") end
    local row = gohelper.cloneInPlace(original, "ultimateSkipAnimation")
    view._ultimateSkipRow = row
    local drop = gohelper.findChildDropdown(row, "dropvideoswitch")
    local template = gohelper.findChild(row, "dropvideoswitch/Template")
    local switch = gohelper.findChild(row, "switch")
    if not drop then error("video-mode dropdown is missing") end
    view._ultimateSkipLabels = setLabels(row, drop, switch)
    if switch then gohelper.setActive(switch, false) end
    gohelper.setActive(row, true)
    row.transform:SetAsFirstSibling()
    view._ultimateSkipDrop = drop
    view._ultimateSkipTemplate = template
    refresh(view)

end

local function guarded(view, callback)
    local ok, reason = pcall(callback, view)
    if not ok then
        if view._ultimateSkipRow then pcall(gohelper.destroy, view._ultimateSkipRow) end
        view._ultimateSkipRow, view._ultimateSkipDrop, view._ultimateSkipTemplate = nil, nil, nil
        view._ultimateSkipLabels = nil
        if logError then logError("Ultimate skip settings unavailable: " .. tostring(reason)) end
    end
end

function SettingsAddon.install(cls)
    if cls._ultimateSkipSettingsInstalled then return end
    cls._ultimateSkipSettingsInstalled = true
    local onInit, addEvents, removeEvents, onOpen = cls.onInitView, cls.addEvents, cls.removeEvents, cls.onOpen
    local onLanguageChanged = cls._onChangeLangTxt
    function cls:onInitView(...)
        local result = onInit(self, ...)
        guarded(self, init)
        return result
    end
    function cls:addEvents(...)
        local result = addEvents(self, ...)
        if self._ultimateSkipDrop then self._ultimateSkipDrop:AddOnValueChanged(onValueChanged, self) end
        return result
    end
    function cls:removeEvents(...)
        if self._ultimateSkipDrop then self._ultimateSkipDrop:RemoveOnValueChanged() end
        return removeEvents(self, ...)
    end
    function cls:onOpen(...)
        local result = onOpen(self, ...)
        if self._ultimateSkipDrop then refresh(self) end
        return result
    end
    function cls:_onChangeLangTxt(...)
        local result
        if onLanguageChanged then result = onLanguageChanged(self, ...) end
        if self._ultimateSkipDrop then guarded(self, refresh) end
        return result
    end
end

return SettingsAddon
