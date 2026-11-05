local prefs = {}
local language = "zh"
GameConfig = {GetCurLangShortcut = function() return language end}
PlayerPrefsHelper = {
    getNumber = function(k, d) return prefs[k] == nil and d or prefs[k] end,
    setNumber = function(k, v) prefs[k] = v end,
    save = function() end,
}
function typeof(x) return x end
TMPro = { TMP_Text = "tmp" }
SLFramework = { LangTxt = "lang" }
UnityEngine = { Object = { Destroy = function() end }, UI = { Text = "text", Image = "image" } }
recthelper = { setHeight = function(tr, n) tr.height = n end,
    getHeight = function(tr) return tr.height end }

local function textItem(value)
    local tr = { IsChildOf = function() return false end }
    return { text = value, transform = tr, gameObject = { GetComponent = function() return nil end } }
end
local title, description = textItem("video title"), textItem("video description")
local drop = { gameObject = { transform = {} }, options = {} }
function drop:ClearOptions() self.options = {} end
function drop:AddOptions(items) for _, item in ipairs(items) do table.insert(self.options, item) end end
function drop:SetValue(v) self.value = v; if self.callback then self.callback(self.handle, v) end end
function drop:AddOnValueChanged(cb, obj) self.callback, self.handle = cb, obj end
function drop:RemoveOnValueChanged() self.callback, self.handle = nil, nil end
local original = {}
local switch = { transform = {} }
local template = { transform = {} }
local content = { transform = {height=1200} }
local viewport = { transform = {height=600, InverseTransformPoint = function(_, p) return p end} }
local primaryArrow = {active=true}
local duplicateArrow = {active=true}
local row = { transform = { SetAsFirstSibling = function(self) self.first = true end } }
function row:GetComponentsInChildren(kind)
    if kind == "tmp" then return { Length = 2, [0] = title, [1] = description } end
    return { Length = 0 }
end
gohelper = {
    findChild = function(obj, path)
        if path == "graphicsScroll/Viewport/Content/videomode" then return original end
        if path == "dropvideoswitch/Template" then return template end
        if path == "switch" then return switch end
        if path == "graphicsScroll/Viewport/Content" then return content end
        if path == "graphicsScroll/Viewport" then return viewport end
        if path == "#go_arrow" then return primaryArrow end
        if path == "GameObject/#go_arrow" then return duplicateArrow end
    end,
    cloneInPlace = function() return row end,
    findChildDropdown = function() return drop end,
    setActive = function(obj, value) obj.active = value end,
    destroy = function(obj) obj.destroyed = true end,
}
SettingsGraphicsFake = {
    onInitView = function(self) self.originalInit = true end,
    addEvents = function(self) self.originalAdd = true end,
    removeEvents = function(self) self.originalRemove = true end,
    onOpen = function(self) self.originalOpen = true end,
    _onChangeLangTxt = function(self) self.originalLanguageChanges = (self.originalLanguageChanges or 0) + 1 end,
}

function test_settings_addon(addon)
    addon.install(SettingsGraphicsFake)
    local view = setmetatable({viewGO = {}}, {__index = SettingsGraphicsFake})
    view:onInitView()
    assert(view.originalInit and view._ultimateSkipRow == row and switch.active == false)
    assert(row.active and row.transform.first and title.text == "至终的仪式动画")
    assert(primaryArrow.active and duplicateArrow.active and not primaryArrow.destroyed and not duplicateArrow.destroyed, "original scroll indicators must remain untouched")
    assert(#drop.options == 2 and drop.value == 0 and template.transform.height == 146)
    view:addEvents()
    drop:SetValue(1)
    assert(prefs.CodexUltimateSkipModeV1 == 2)
    view:onOpen()
    assert(view.originalOpen and drop.value == 1 and prefs.CodexUltimateSkipModeV1 == 2)
    assert(primaryArrow.active and duplicateArrow.active, "opening must leave the original scroll indicators untouched")
    drop:SetValue(2)
    assert(prefs.CodexUltimateSkipModeV1 == 2)
    view:onOpen()
    assert(drop.value == 1, "invalid dropdown indices cannot create another mode")
    view:removeEvents()
    assert(view.originalRemove and not drop.callback)


end

function test_settings_localization(addon)
    prefs.CodexUltimateSkipModeV1 = 2
    addon.install(SettingsGraphicsFake)
    local view = setmetatable({viewGO = {}}, {__index = SettingsGraphicsFake})
    view:onInitView()
    view:addEvents()
    local cases = {
        {"zh", "至终的仪式动画", "选择大招动画的播放方式", "始终播放", "始终跳过"},
        {"tw", "至終的儀式動畫", "選擇大招動畫的播放方式", "始終播放", "始終跳過"},
    }
    for _, locale in ipairs({"en", "jp", "kr", "de", "fr", "thai", "unknown"}) do
        cases[#cases + 1] = {locale, "Ultimate animation", "Choose how Ultimate animations are played.", "Always play", "Always skip"}
    end
    for _, case in ipairs(cases) do
        language = case[1]
        view:_onChangeLangTxt()
        assert(title.text == case[2] and description.text == case[3], "labels must match the game text language")
        assert(#drop.options == 2 and drop.options[1] == case[4] and drop.options[2] == case[5])
        assert(drop.value == 1 and prefs.CodexUltimateSkipModeV1 == 2, "language refresh must preserve always-skip")
    end
    assert(view.originalLanguageChanges == #cases, "retain the original language refresh")
    prefs.CodexUltimateSkipModeV1 = 1
    view:onOpen()
    assert(drop.value == 0, "removed legacy mode displays always-play")
    drop:SetValue(1)
    assert(prefs.CodexUltimateSkipModeV1 == 2)
    drop:SetValue(0)
    assert(prefs.CodexUltimateSkipModeV1 == 0)
    view:removeEvents()
end
