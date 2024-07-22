local a = require("gen.misc").alpha
Name = "yue-sun"
Style = "light"

local colorMap = {
    darkerCreamWhite = "#BCA88F",
    darkCreamWhite = "#e5dfd6",
    creamWhite = "#F2ECE3",
    white = "#FFFFFF",
    gray1 = "#C9C9C9",
    gray2 = "#616161",
    gray3 = "#3C3C3C",
    darkGray = "#2B2B2B",
    veryDarkGray = "#3C3C3C",
    red = "#cf1f4c",
    orange = "#cc7158",
    yellow = "#d6893c",
    green = "#699058",
    lightCyan = "#518C7B",
    cyan = "#55818F",
    blue = "#635ab7",
    purple = "#915d8b",
    pink = "#c14b77",
}


-- Color Table
return {
    name = Name,
    style = Style,
    cursorColor = colorMap.darkerCreamWhite .. a("80"),
    textHighlightColor = colorMap.darkerCreamWhite .. a("20"),
    lineHighlightBackground = colorMap.darkerCreamWhite .. a("10"),
    editorForeground = colorMap.gray3,
    editorBackground = colorMap.creamWhite,
    explorerForeground = colorMap.gray3,
    explorerBackground = colorMap.darkCreamWhite,
    statusBarBackground = colorMap.creamWhite,
    statusBarForeground = colorMap.darkGray,
    lineNumber = colorMap.darkerCreamWhite,
    statusLineForeground = colorMap.darkGray,
    statusLineBackground = colorMap.creamWhite,
    activityBarBadgeBackground = colorMap.green .. a("80"),

    variables = colorMap.gray3,
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
