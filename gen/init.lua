local gen = require("gen.generator")
local config = require("gen.config")
local utils = require("gen.misc")

return {
    config = config.config,
    generators = gen,
    colors = config.themes,
    utils = utils,
}
