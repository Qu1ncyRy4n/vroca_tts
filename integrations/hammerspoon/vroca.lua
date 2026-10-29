-- Nix substitutes absolute paths; Hammerspoon need not inherit a shell PATH.
local M = { hotkeys = {}, tasks = {} }
local tts = "@TTS@"
local app = "@APP@"

-- Match the developer's Linux/Speechify layout, with Option replacing Super.
M.defaultBindings = {
    { mods={"alt"}, key="z", args={"read"} },
    { mods={"alt", "shift"}, key="z", panel=true },
    { mods={"alt"}, key="x", args={"toggle"} },
    { mods={"alt"}, key="c", args={"back"} },
    { mods={"alt"}, key="v", args={"next"} },
    { mods={"alt", "shift"}, key="v", args={"faster"} },
    { mods={"alt", "shift"}, key="c", args={"slower"} },
}

function M.command(arguments)
    local task
    task = hs.task.new(tts, function(code, stdout, stderr)
        M.tasks[task] = nil
        if code ~= 0 then
            hs.alert.show("Vroca command failed; check the daemon and tts log")
        end
    end, arguments)
    if not task then
        hs.alert.show("Cannot create Vroca command")
        return false
    end
    M.tasks[task] = true -- Retain tasks until their asynchronous completion.
    if not task:start() then
        M.tasks[task] = nil
        hs.alert.show("Cannot start Vroca command")
        return false
    end
    return true
end

function M.panel()
    if not hs.application.launchOrFocus(app) then
        hs.alert.show("Cannot open Vroca panel")
        return false
    end
    return true
end

function M.stop()
    for _, key in ipairs(M.hotkeys) do key:delete() end
    M.hotkeys = {}
end

-- Loading binds nothing. setup() enables the approved defaults; setup({})
-- enables nothing, and a supplied table replaces the defaults.
-- Each entry has mods, key, and either args (a tts argument list) or panel=true.
function M.setup(bindings)
    M.stop()
    for _, binding in ipairs(bindings or M.defaultBindings) do
        local action = binding.panel and M.panel or function()
            M.command(binding.args)
        end
        local key = hs.hotkey.bind(binding.mods, binding.key, action)
        if not key then
            M.stop()
            error("Cannot bind Vroca shortcut: " .. binding.key)
        end
        table.insert(M.hotkeys, key)
    end
    return M
end

return M
