local a = require("gen.misc").alpha
Name = "yue-newmoon"
Style = "dark"

-- require("gen.misc").print_colormap(colorMap)
local colorMap = {
	white = "#dfdfdf",
	gray1 = "#bcbcbc",
	gray2 = "#7a7a7a",
	gray3 = "#1a1a1a",
	darkGray = "#000000",
	darkerGray = "#000000",
	red = "#ff91a6",
	orange = "#ffd8c7",
	yellow = "#fff5c2",
	green = "#e2ffcc",
	lightCyan = "#ceffff",
	cyan = "#d5ffff",
	blue = "#ddccff",
	purple = "#f9bef0",
	pink = "#ffb7e0",
}

return {
	name = Name,
	style = Style,
	cursorColor = colorMap.white,
	textHighlightColor = colorMap.purple .. a("30"),
	lineHighlightBackground = colorMap.purple .. a("20"),
	editorForeground = colorMap.white,
	editorBackground = colorMap.darkGray,
	explorerForeground = colorMap.gray1,
	explorerBackground = colorMap.darkerGray,
	statusBarBackground = colorMap.darkGray,
	statusBarForeground = colorMap.white,
	lineNumber = colorMap.gray2,
	statusLineForeground = colorMap.white,
	statusLineBackground = colorMap.darkGray,
	activityBarBadgeBackground = colorMap.darkerGray,

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
