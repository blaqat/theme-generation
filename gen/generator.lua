local json = require('dkjson')
local themes = require("gen.config")
local exports = {}

local function generate_vs_colors(colors)
    local theme = {
        name = colors.name,
        colors = {
            ["editor.background"] = colors.editorBackground,
            ["editor.foreground"] = colors.editorForeground,
            ["activityBarBadge.background"] = colors.activityBarBadgeBackground,
            ["sideBarTitle.foreground"] = colors.explorerForeground,
            ["statusBar.background"] = colors.statusBarBackground,
            ["statusBar.foreground"] = colors.statusBarForeground,
            ["statusBar.noFolderBackground"] = colors.statusBarBackground,
            ["statusBar.noFolderForeground"] = colors.statusBarForeground,
            ["sideBar.background"] = colors.explorerBackground,
            ["sideBar.foreground"] = colors.explorerForeground,
            ["list.activeSelectionBackground"] = colors.explorerBackground,
            ["list.activeSelectionForeground"] = colors.explorerForeground,
            -- ["list.inactiveSelectionBackground"] = colors.explorerBackground,
            ["list.inactiveSelectionForeground"] = colors.explorerForeground,
            ["list.hoverBackground"] = colors.explorerBackground,
            ["list.hoverForeground"] = colors.explorerForeground,
            ["editorLineNumber.foreground"] = colors.lineNumber,
            ["editorLineNumber.activeForeground"] = colors.lineNumber,
            ["editor.lineHighlightBackground"] = colors.lineHighlightBackground,
            -- ["editor.lineHighlightBorder"] = colors.lineHighlightBorder,
            ["panel.background"] = colors.explorerBackground,
            ["panel.border"] = colors.lineHighlightBackground,
            ["editorCursor.foreground"] = colors.cursorColor,
            ["editor.selectionBackground"] = colors.textHighlightColor,
            ["terminal.background"] = colors.explorerBackground,
            ["titleBar.activeBackground"] = colors.editorBackground,
            ["titleBar.activeForeground"] = colors.editorForeground,
            ["titleBar.inactiveBackground"] = colors.explorerBackground,
            ["titleBar.inactiveForeground"] = colors.explorerForeground,
            ["list.highlightForeground"] = colors.activityBarBadgeBackground,
            ["quickInputList.focusBackground"] = colors.textHighlightColor,
            ["quickInputList.focusForeground"] = colors.editorForeground,
            ["quickInput.background"] = colors.explorerBackground,
            ["list.background"] = colors.explorerBackground,
            ["list.focusBackground"] = colors.textHighlightColor,
            ["list.focusForeground"] = colors.editorForeground,
            ["list.inactiveSelectionBackground"] = colors.lineHighlightBackground,
            ["editor.wordHighlightBackground"] = colors.textHighlightColor,
            -- ["editor.wordHighlightBackground"] = colors.wordHighlightBackground,
            -- ["editor.wordHighlightStrongBackground"] = colors.textHighlightStrongColor,
            ["editor.selectionHighlightBackground"] = colors.textSelectionBackground,
            -- ["editor.selectionHighlightBorder"] = colors.textHighlightBordeHighlightBorder,
            ["editorCursor.background"] = colors.cursorHighlightColor,
            -- ["editor.selectionBackground"] = colors.selectionBackground,
            ["editor.inactiveSelectionBackground"] = colors.textSelectionBackground,
            ["editor.hoverHighlightForeground"] = colors.editorBackground,
            ["editorHoverWidget.background"] = colors.explorerBackground,
            ["editorHoverWidget.border"] = colors.textHighlightColor,
            ["editorHoverWidget.foreground"] = colors.editorForeground
        },
        tokenColors = {
            {
                name = "Comment",
                scope = { "comment", "punctuation.definition.comment" },
                settings = {
                    foreground = colors.comment
                }
            },
            {
                name = "Keyword, Storage",
                scope = { "keyword", "storage.type", "storage.modifier", "keyword.other.fn", "keyword.control" },
                settings = {
                    foreground = colors.keyword
                }
            },
            {
                name = "Operator, Misc",
                scope = {
                    "constant.other.color", "punctuation", "meta.tag", "punctuation.definition.tag",
                    "punctuation.separator.inheritance.php", "punctuation.definition.tag.html",
                    "punctuation.definition.tag.begin.html", "punctuation.definition.tag.end.html",
                    "punctuation.section.embedded", "keyword.other.template", "keyword.other.substitution"
                },
                settings = {
                    foreground = colors.operator
                }
            },
            {
                name = "Operator, Specific",
                scope = { "keyword.operator" },
                settings = {
                    foreground = colors.operatorSpecific
                }
            },
            {
                name = "Operator, Rust Specific",
                scope = { "source.rust keyword.operator.borrow.and.rust" },
                settings = {
                    foreground = colors.operatorRustSpecific
                }
            },
            {
                name = "Tag",
                scope = { "entity.name.tag", "meta.tag.sgml", "markup.deleted.git_gutter" },
                settings = {
                    foreground = colors.tag
                }
            },
            {
                name = "Function, Builtin",
                scope = {
                    "support.function -support.function.any-method", "storage.type.built-in",
                    "entity.name.function.support.builtin", "entity.name.function.macro",
                    "variable.language.this", "entity.name.function.member"
                },
                settings = {
                    foreground = colors.functionBuiltin
                }
            },
            {
                name = "Function, Special Method",
                scope = {
                    "entity.name.function", "meta.function-call", "variable.function", "keyword.other.special-method"
                },
                settings = {
                    foreground = colors.functionSpecialMethod
                }
            },
            {
                name = "Built-in Function, Class",
                scope = { "support.function.builtin", "support.class.builtin" },
                settings = {
                    foreground = colors.functionBuiltin
                }
            },
            {
                name = "Variables",
                scope = { "variable", "string constant.other.placeholder" },
                settings = {
                    foreground = colors.variables
                }
            },
            {
                name = "Global Variables",
                scope = { "variable.global", "variable.language" },
                settings = {
                    foreground = colors.globalVariables
                }
            },
            {
                name = "Constant, Boolean",
                scope = {
                    "constant", "constant.numeric", "constant.language", "support.constant",
                    "constant.character", "constant.escape", "variable.parameter", "keyword.other.unit",
                    "keyword.other", "constant.other.color", "keyword.constant.bool", "boolean"
                },
                settings = {
                    foreground = colors.constantBoolean
                }
            },
            {
                name = "String",
                scope = { "string" },
                settings = {
                    foreground = colors.string
                }
            },
            {
                name = "Number",
                scope = { "constant.numeric" },
                settings = {
                    foreground = colors.number
                }
            },
            {
                name = "Type",
                scope = {
                    "entity.name.type", "support.type", "support.class", "support.other.namespace.use.php",
                    "meta.use.php", "support.other.namespace.php", "markup.changed.git_gutter",
                    "support.type.sys-types", "keyword.type", "source.go storage.type"
                },
                settings = {
                    foreground = colors.type
                }
            },
            {
                name = "Identifier",
                scope = { "identifier" },
                settings = {
                    foreground = colors.identifier
                }
            },
            {
                name = "Line Number",
                scope = { "line.number" },
                settings = {
                    foreground = colors.lineNumber
                }
            },
            {
                name = "Status Line",
                scope = { "statusBar.background", "statusBar.foreground" },
                settings = {
                    foreground = colors.statusLineForeground,
                    background = colors.statusLineBackground
                }
            },
            {
                name = "Todo",
                scope = { "todo" },
                settings = {
                    foreground = colors.todo
                }
            },
            {
                name = "Error",
                scope = { "invalid", "invalid.illegal" },
                settings = {
                    foreground = colors.error
                }
            },
            {
                name = "Special Character",
                scope = { "constant.character.escape", "variable.language.self" },
                settings = {
                    foreground = colors.specialCharacter
                }
            },
            {
                name = "Function, Other",
                scope = {
                    "source.ts storage.type.function",
                    "source.rust keyword.other.fn",
                    "source.lua keyword.control",
                    "source.go keyword.function",
                    "keyword.operator.ternary"
                },
                settings = {
                    foreground = colors.functionDefinition
                }
            },
            {
                name = "Preprocessor",
                scope = { "entity.name.function.preprocessor" },
                settings = {
                    foreground = colors.preprocessor
                }
            },
            {
                name = "Comparison",
                scope = { "keyword.operator.comparison", "keyword.operator.relational" },
                settings = {
                    foreground = colors.comparison
                }
            }
        }
    }

    local themeJson = json.encode(theme, { indent = true })

    local file = io.open("./themes/" .. colors.name .. "-color-theme.json", "w")
    if not file then
        print("Error: Could not open file for writing")
        return
    end
    file:write(themeJson)
    file:close()
end

local function generate_vs_package()
    local package = {
        name = themes.config.name,
        displayName = themes.config.displayName,
        description = themes.config.description,
        version = themes.config.version or "1.0.0",
        publisher = "nukarma",
        engines = {
            vscode = "^1.0.0"
        },
        categories = {
            "Themes"
        },
        contributes = {
            themes = themes.config.themes(themes.themes)
        }
    }

    local packageJson = json.encode(package, { indent = true })

    local file = io.open("./package.json", "w")
    if not file then
        print("Error: Could not open file for writing")
        return
    end
    file:write(packageJson)
    file:close()
end

function exports.GenerateVS()
    for _, theme in ipairs(themes.themes) do
        generate_vs_colors(theme)
        print("Generated theme: " .. theme.name)
    end

    generate_vs_package()
end

return exports
