local export = {}

function export.alpha(num)
    -- Map 0 .. 100 to 00 .. FF
    num = tonumber(num)
    if num == 0 then
        return "00"
    end
    return string.format("%X", math.floor(num / 100 * 255))
end

function export.split(str, delimiter, r)
    delimiter = delimiter or "#"
    r = r or true
    local result = {}
    for match in (str .. delimiter):gmatch("(.-)" .. delimiter) do
        if match ~= "" then
            match = r and delimiter .. match or match
            table.insert(result, match)
        end
    end
    return result
end

function export.print_colormap(colorMap)
    print("local colorMap = {")
    for k, v in pairs(colorMap) do
        print("\t" .. k .. " = \"" .. v .. "\",")
    end
    print("}")
end

function export.print_colors(map, ...)
    local colors = { ... }
    local s = ""
    for _, color in ipairs(colors) do
        s = s .. map[color]
    end
    print(s)
end

string.split = export.split

return export
