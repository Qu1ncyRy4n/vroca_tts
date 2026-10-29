-- No Hammerspoon process, global hotkeys, clipboard, or daemon is used.
local tasks, keys, alerts = {}, {}, {}
hs = {
    alert = { show = function(message) table.insert(alerts, message) end },
    application = { launchOrFocus = function(path)
        assert(path:match("/Applications/Vroca.app$"))
        return true
    end },
    hotkey = { bind = function(mods, key, callback)
        local binding = { mods=mods, key=key, callback=callback }
        function binding:delete() self.deleted = true end
        table.insert(keys, binding)
        return binding
    end },
    task = { new = function(path, callback, args)
        assert(path:match("/bin/tts$"))
        local task = { callback=callback, args=args }
        function task:start() return self end
        table.insert(tasks, task)
        return task
    end },
}
local module = dofile(assert(arg[1], "pass packaged vroca.lua"))
assert(#keys == 0, "loading must not bind keys")
module.setup({
    { mods={"ctrl", "alt", "cmd"}, key="r", args={"read"} },
    { mods={"ctrl", "alt", "cmd"}, key="p", panel=true },
})
keys[1].callback()
assert(tasks[1].args[1] == "read")
assert(module.tasks[tasks[1]], "retain running task")
tasks[1].callback(0, "ok", "")
assert(not module.tasks[tasks[1]], "release completed task")
keys[2].callback()
module.command({"speed", "1.2"})
assert(tasks[2].args[2] == "1.2", "forward argument boundaries")
tasks[2].callback(1, "", "connection failed")
assert(#alerts == 1, "surface process failure")
module.setup({})
assert(keys[1].deleted and keys[2].deleted, "setup replaces own keys")
assert(#module.hotkeys == 0)
module.setup()
assert(#module.hotkeys == 7, "enable approved Option bindings only on setup")
local expected = {"read", "panel", "toggle", "back", "next", "faster", "slower"}
for i, binding in ipairs(module.defaultBindings) do
    assert(binding.mods[1] == "alt")
    assert((binding.panel and "panel" or binding.args[1]) == expected[i])
end
module.stop()
hs.task.new = function() return nil end
assert(module.command({"stop"}) == false)
hs.task.new = function() return { start=function() return false end } end
assert(module.command({"stop"}) == false)
assert(next(module.tasks) == nil, "failed starts must not leak tasks")
hs.hotkey.bind = function() return nil end
assert(not pcall(module.setup, {{mods={}, key="r", args={"read"}}}))
print("Hammerspoon dispatch, task lifetime, key cleanup, and failures: passed")
