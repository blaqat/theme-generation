local lfs = require("lfs")

local config = {
    name = "yue",
    displayName = "Yue Theme",
    description = "Theme inspired by 'moon' from the Moonscript website.",
    version = "1.5.0",
}

local function join_paths(...)
    local sep = package.config:sub(1, 1)
    return table.concat({ ... }, sep)
end

local colors = {}
local colors_dir = join_paths(lfs.currentdir(), "gen/colors")

for file in lfs.dir(colors_dir) do
    if file ~= "." and file ~= ".." then
        local file_path = join_paths(colors_dir, file)
        if file:match("%.lua$") then
            local color_module = file:sub(1, -5) -- Remove ".lua" extension
            local color = require("gen/colors." .. color_module)
            table.insert(colors, color)
        end
    end
end

config.themes = function(themes)
    local t = {}

    for i, theme in ipairs(themes) do
        table.insert(t, {
            label = theme.name,
            uiTheme = theme.style == "dark" and "vs-dark" or "vs",
            path = "./themes/" .. theme.name .. "-color-theme.json",
        })
    end

    return t
end

return {
    config = config,
    themes = colors
}
