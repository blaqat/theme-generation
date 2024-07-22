local theme = require("gen")

-- get run arguments for generators (vs = generators.GenerateVS, vim = generators.GenerateVim)

local generator = arg[1] or "vs"

-- generate theme
if not theme.generators["Generate" .. generator:upper()] then
    error("Invalid generator: " .. generator)
end

theme.generators["Generate" .. generator:upper()]()
