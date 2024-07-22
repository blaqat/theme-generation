local a = require("gen.misc").alpha
Name = "yue-fullmoon"
Style = "dark"

-- local darks = ("#414063#28293a#191b28"):split("#")
-- local darks = ("#272748#1a1b2b#111320"):split("#")
-- local darks = ("#1a1b3b#131424#0c0d1c"):split("#")

local colorMap = {
	white = "#dfe0f2",
	gray1 = "#bcbdd1",
	gray2 = "#7a7b91",
	gray3 = "#414963",
	darkGray = "#282d3b",
	darkerGray = "#1d212e",
	-- gray3 = darks[1],
	-- darkGray = darks[2],
	-- darkerGray = darks[3],
	red = "#ff657f",
	orange = "#ffa697",
	yellow = "#ffd5a3",
	green = "#c7efb2",
	lightCyan = "#ccffff",
	cyan = "#aee1e0",
	blue = "#afa0ff",
	purple = "#cc94c5",
	pink = "#ff89b2",
}

-- Color Table
return {
	name = Name,
	style = Style,
	cursorColor = colorMap.white,
	textHighlightColor = colorMap.gray3 .. a("80"),
	lineHighlightBackground = colorMap.gray3 .. a("50"),
	editorForeground = colorMap.white,
	editorBackground = colorMap.darkGray,
	explorerForeground = colorMap.gray1,
	explorerBackground = colorMap.darkerGray,
	statusBarBackground = colorMap.darkGray,
	statusBarForeground = colorMap.white,
	lineNumber = colorMap.gray2,
	statusLineForeground = colorMap.white,
	statusLineBackground = colorMap.darkGray,
	activityBarBadgeBackground = colorMap.blue .. a("80"),

	variables = colorMap.white,
	globalVariables = colorMap.cyan,
	preprocessor = colorMap.cyan,
	constantBoolean = colorMap.lightCyan,
	keyword = colorMap.purple,
	number = colorMap.blue,
	type = colorMap.cyan,
	identifier = colorMap.orange,
	string = colorMap.yellow,
	specialCharacter = colorMap.orange,

	operator = colorMap.pink,
	tag = colorMap.cyan,
	comparison = colorMap.orange,
	operatorSpecific = colorMap.pink,
	operatorRustSpecific = colorMap.yellow,

	comment = colorMap.gray2,
	todo = colorMap.gray2,

	functionBuiltin = colorMap.orange,
	functionDefinition = colorMap.purple,
	functionSpecialMethod = colorMap.green,

	error = colorMap.red,
}
