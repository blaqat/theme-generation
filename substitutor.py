import toml
import json
import re
import sys
import os
from pathlib import Path
import argparse
from collections import Counter
from copy import deepcopy

PRINT = False

# Updated patterns to match list variables
FILE_PATTERN = r'"(\$[a-zA-Z0-9._]+(?:\s*,\s*\$[a-zA-Z0-9._]+)*)"'
REC_PATTERN = r'(\$[a-zA-Z0-9._]+(?:\s*,\s*\$[a-zA-Z0-9._]+)*)'
ALPHA_PATTERN = r'\.\.([a-zA-Z0-9][a-zA-Z0-9])'

DELETE = '^SUBDELETE'

def load_toml(file_path):
    with open(file_path, 'r') as file:
        return toml.load(file)

def listify(value: list) -> str:
    return "^SUBLIST" + str(value).replace(" ", "").replace("'", "")

def is_listified(value: str) -> bool:
    if not isinstance(value, str):
        return False
    return value.startswith("^SUBLIST")

def delistify(value: str) -> list:
    return value[9:-1].split(",")

def resolve_variable(var_name, toml_data, had_alpha=False):
    if not had_alpha:
        a = re.search(ALPHA_PATTERN, var_name)
        alpha = a.group(1) if a else False

        if alpha:
            var_name = var_name.replace(f"..{alpha}", '')
            value = resolve_variable(var_name, toml_data, had_alpha=True)
            while isinstance(value, str) and value.startswith('$'):
                value = resolve_variable(value[1:], toml_data, had_alpha=True)
            if not value: return None
            if re.search(ALPHA_PATTERN, value): return value
            value = value + alpha if value else None
            return value

    parts = var_name.split('.')
    current = toml_data
    for part in parts:
        if isinstance(current, dict) and part in current:
            current = current[part]
        else:
            return None

    return current

def substitute_variables(text, toml_data, recursive=False, quotes=True):
    def replacer(match):
        var_names = [name.strip() for name in match.group(1).split(',')]

        for var_name in var_names:
            if var_name.startswith('$'):
                var_name = var_name[1:]  # Remove the leading $
            value = resolve_variable(var_name, toml_data)

            if value is None and var_name == var_names[-1]:
                # Try to resolve parent if child not found
                parent = var_name.split('.')[0]
                value = resolve_variable(parent, toml_data)

            if value is None or isinstance(value, bool):
                continue  # Try next variable in the list

            # If value isn't a number or string, skip to next variable
            if not isinstance(value, (str, int, float, list)):
                continue

            # Recursive substitution for list values
            if isinstance(value, list):
                value = [
                    substitute_variables(item, toml_data, recursive=True, quotes=False)
                    for item in value
                ]
                return json.dumps(value)

            # Recursive substitution if value is a string and contains a variable
            if isinstance(value, str) and '$' in value:
                return substitute_variables(value, toml_data, recursive=True, quotes=quotes)

            # Return the value as-is (without quotes) if it's not a string
            if not isinstance(value, str):
                return json.dumps(value)

            # For strings, return with quotes
            return json.dumps(value) if quotes else value

        return 'null'  # If no valid value found in the list

    if recursive:
        pattern = REC_PATTERN
    else:
        pattern = FILE_PATTERN

    return re.sub(pattern, replacer, text)

def process_json_template(template_path, toml_files):
    with open(template_path, 'r') as file:
        template = json.load(file)

    processed_themes = []

    for toml_path in toml_files:
        toml_data = load_toml(toml_path)
        theme = deepcopy(template['themes'][0])

        # Apply deletions
        if 'deletions' in toml_data and 'keys' in toml_data['deletions'] and toml_data['deletions']['keys']:
            for full_key in toml_data['deletions']['keys']:
                keys = parse_full_key(full_key)
                val = theme
                for key in keys[:-1]:
                    if key not in val:
                        continue # TODO: FIX THIS
                    val = val[key]
                if keys[-1] in val:
                    del val[keys[-1]]

        theme_template = json.dumps(theme)


        processed_theme = json.loads(substitute_variables(theme_template, toml_data))

        # Apply overrides
        if 'overrides' in toml_data:
            apply_direct_overrides(processed_theme, toml_data, toml_data['overrides'])
        if 'overrides-regex' in toml_data:
            apply_regex_overrides(processed_theme, toml_data, toml_data['overrides-regex'])

        if 'syntax-overrides' in toml_data:
            apply_overrides(processed_theme['style']['syntax'], toml_data, toml_data['syntax-overrides'])
        if 'syntax-overrides-regex' in toml_data:
            apply_regex_overrides(processed_theme['style']['syntax'], toml_data, toml_data['syntax-overrides-regex'])

        if 'stlye-overrides' in toml_data:
            apply_overrides(processed_theme['style'], toml_data, toml_data['style-overrides'])
        if 'style-overrides-regex' in toml_data:
            apply_regex_overrides(processed_theme['style'], toml_data, toml_data['style-overrides-regex'])


        processed_themes.append(processed_theme)

    template['themes'] = processed_themes
    return template

def wildcard_to_regex(pattern: str) -> str:
    escaped_pattern = re.escape(pattern)
    escaped_pattern = escaped_pattern.replace(r'\*\*\*', '.*')
    escaped_pattern = escaped_pattern.replace(r'\.\*\*', r'\..*')
    escaped_pattern = escaped_pattern.replace(r'\*\*\.', r'.*\.')
    escaped_pattern = escaped_pattern.replace(r'\*', r'\w*')
    return f'^{escaped_pattern}$'

def match_wildcard(pattern, key):
    return re.match(wildcard_to_regex(pattern), key)

def find_matching_keys(data, pattern):
    # print(data.keys())
    return [key for key in data.keys() if match_wildcard(pattern, key)]

def apply_overrides(theme, toml_data, overrides):
    for override_key, override_value in overrides.items():
        if override_value and "$" in override_value:
            override_value = resolve_variable(override_value[1:], toml_data)

        matching_keys = find_matching_keys(theme, override_key)
        for key in matching_keys:
            if isinstance(override_value, dict):
                theme[key].update(override_value)
            elif isinstance(override_value, list):
                print("List overrides not supported")
                print(key, override_value)
            elif type(theme[key]) == type(override_value) or theme[key] is None:
                theme[key] = override_value
            elif not override_value:
                theme[key] = None
            else:
                print(f"Override value type mismatch for key {key}")
    return theme

def parse_full_key(full_key: str) -> list:
    parts = full_key.split('.')
    keys = []
    i = 0
    while i < len(parts):
        part = parts[i]
        if part.startswith('['):
            while not part.endswith(']'):
                i += 1
                part += '.' + parts[i]
            keys.append(part[1:-1])
        elif part.isdigit():
            keys.append(int(part))
        else:
            keys.append(part)
        i += 1

    return keys


def apply_direct_overrides(theme, toml_data, overrides):
    # Direct overrides are named as a.b.c: value in the toml file
    # This each a. could either be the key of a dictionary e.g "syntax.comment" => theme["syntax"]["comment"]
    # or lists "players.0.selection" => theme["players"][0]["selection"]
    # or the begining of the name of normal key e.g "style.[terminal.ansi.white]" => theme["style"]["terminal.ansi.white"]
    for override_key, override_value in overrides.items():
        if isinstance(override_value, str) and "$" in override_value:
            override_value = resolve_variable(override_value[1:], toml_data)
        if not "." in override_key:
            theme[override_key] = override_value
            continue

        parts = parse_full_key(override_key)

        if not parts:
            continue

        current = theme
        part = None
        for i, part in enumerate(parts):
            next = parts[i + 1] if i < len(parts) - 1 else None
            if isinstance(current, dict) and part not in current:
                if i == len(parts) - 1:
                    current[part] = None
                else:
                    if next.isdigit():
                        current[part] = []
                    else:
                        current[part] = {}
            elif isinstance(current, list) and len(current) <= part:
                while len(current) <= part:
                    if next and next.isdigit():
                        current.append([])
                    elif next and next.isdigit():
                        print(part, parts[i])
                        current.append({})
                    elif part == len(current):
                        current.append({})
                    else:
                        current.append(override_value)
            next = current[part]
            if isinstance(next, list) or isinstance(next, dict):
                current = next
            else:
                break

        if override_value is False:
            override_value = None

        if part and part in current:
            current[part] = override_value
        elif isinstance(current, list) and isinstance(part, int):
            if part > len(current):
                current.append(override_value)
            else:
                current[part] = override_value
        else:
            print("Could not find key", override_key, part, current)
            print(type(part), type(current))
            print("\n\n")

    return theme


def apply_regex_overrides(theme, toml_data, regex_overrides):
    for regex_pattern, override_value in regex_overrides.items():
        if override_value and "$" in override_value:
            override_value = resolve_variable(override_value[1:], toml_data)
        elif not override_value:
            override_value = None
        for key in theme.keys():
            if re.match(regex_pattern, key):
                theme[key] = override_value
    return theme

def reverse_process(template_path, final_theme_path, threshold=1):
    global PRINT
    with open(template_path, 'r') as f:
        template = json.load(f)

    with open(final_theme_path, 'r') as f:
        final_theme = json.load(f)

    paths = []

    for i, theme in enumerate(final_theme['themes']):
        templ = None
        if len(template['themes']) != len(final_theme['themes']):
            templ = template['themes'][0]
        else:
            templ = template['themes'][i]

        delete = get_delete_keys(templ, theme, "")

        name, Data = extract_variables(templ, theme)
        variables, overrides, not_done = Data["variables"], Data["overrides"], Data["not_done"]
        var_map = Data["var_map"]

        # print(Data["full_name"])

        if PRINT:
            print(Data["var_map"])
            print("Name:\t", name)
            print("-"*50)
            print("Trash Bin:\t", not_done)
            print("-"*50)

        color_counts, variables, overrides = handle_colors(variables, overrides, var_map, threshold)

        if PRINT:
            print("Color Counts:\t", color_counts)
            print("-"*20)
            print("Variables:\t", variables)
            print("-"*20)
            print("Overrides:\t", overrides)
            print("-"*20)


        output_toml_path = final_theme_path.with_name(f'{name}.toml')
        generate_toml(variables, overrides, color_counts, var_map, output_toml_path, delete)

        paths.append(output_toml_path)

    return paths

def get_delete_keys(template, final_theme, prefix=""):
    delete_keys = []
    for key in template.keys():
        string_key = f"[{key}]" if "." in key else key
        full_key = f"{prefix}.{string_key}" if prefix else key
        if key not in final_theme:
            delete_keys.append(full_key)
            continue
        if isinstance(template[key], dict):
            delete_keys += get_delete_keys(template[key], final_theme[key], full_key)
        elif isinstance(template[key], list):
            for i, item in enumerate(template[key]):
                if i >= len(final_theme[key]):
                    final_theme[key].append(item)
                elif isinstance(item, dict):
                    delete_keys += get_delete_keys(item, final_theme[key][i], full_key)

    return delete_keys

def extract_variables(template, final_theme, Data=None, prefix=""):
    if not Data:
        Data = {}
        Data["variables"] = {} # Stored as {variable: {value: count}}
        Data["overrides"] = {} # Stored as {variable: value}
        Data["not_done"] =  {} # Stored as {final_theme_key: (value, var_name)}
        Data["var_map"] = {} # Stored as {var_name: {final_theme_key: value}}
        Data["full_name"] = {}


    variables = Data["variables"]
    overrides = Data["overrides"]
    not_done = Data["not_done"]
    full_name_map = Data["full_name"]


    name = final_theme.get('name')


    def parse_variable(var_name):
        """
        Parses a variable name and returns a list of variables
        """
        if not var_name:
            return [var_name]
        variables = [ found and found for found in re.findall(r"(\$\w+(?:.\w+)*)", var_name)]

        return variables


    def handle_variable(key, template_var_name, value):
        """
        Handles a variable and updates the variables, overrides,
        and not_done dictionaries
        """
        if template_var_name.startswith("$"):
            if template_var_name in variables:
                if value in variables[template_var_name]:
                    variables[template_var_name][value] += 1
                else:
                    variables[template_var_name][value] = 1
            else:
                if isinstance(value, list):
                    value = listify(value)
                variables[template_var_name] = {value: 1}
        elif template_var_name in final_theme:
            if key in overrides:
                raise ValueError(f"Key {key} already exists in overrides")
            else:
                overrides[key] = final_theme[template_var_name]
        else:
            not_done[key] = (value, template_var_name)

    def extract(templ_partial, final_partial, Data, current_prefix):
        # Iterates through the final theme and template to extract variables
        for key, value in final_partial.items():
            string_key = f"[{key}]" if "." in key else key
            full_key = f"{current_prefix}.{string_key}" if current_prefix else key
            if isinstance(value, dict) and isinstance(templ_partial.get(key), dict):
                extract(templ_partial[key], value, Data, full_key)
            elif isinstance(value, list) and isinstance(templ_partial.get(key), list):
                for i, item in enumerate(value):
                    full_key_list = f"{full_key}.{i}"
                    if i < len(templ_partial[key]):
                        if isinstance(item, dict):
                            extract(templ_partial[key][i], item, Data, full_key_list)
                        else:
                            full_name_map[full_key_list] = (item, templ_partial[key][i])
                            vars = parse_variable(templ_partial[key][i])
                            for template_var_name in vars:
                                if type(item) == type(template_var_name):
                                    if item != template_var_name:
                                        # print(f"{key}:", item, "|", template_var_name)
                                        # print(prefix, key, full_key, f"{prefix}.[{key}]")
                                        handle_variable(full_key, template_var_name, item)
                                else:
                                    not_done[full_key_list] = (item, template_var_name)
                    else:
                        full_name_map[full_key_list] = (item, None)
                        not_done[full_key_list] = (item, None)
            elif isinstance(value, list) and isinstance(templ_partial.get(key), str):
                handle_variable(full_key, templ_partial.get(key), value)
            else:
                full_name_map[full_key] = (value, templ_partial.get(key, None))
                vars = parse_variable(templ_partial.get(key, None))
                for template_var_name in vars:
                    if type(value) == type(template_var_name):
                        if value != template_var_name:
                            # print(f"{key}:", value, "|", template_var_name)
                            # print(prefix, key, full_key, f"{prefix}.[{key}]")
                            handle_variable(full_key, template_var_name, value)
                    elif isinstance(value, int) and isinstance(template_var_name, str):
                        handle_variable(full_key, template_var_name, value)
                        # handle_variable(full_key, template_var_name, value)
                    # elif isinstance(value, bool) and isinstance(template_var_name, str):
                    #     print(full_key, value, template_var_name)
                    #     handle_variable(full_key, template_var_name, value)
                    else:
                        # print(full_key, value, template_var_name)
                        not_done[full_key] = (value, template_var_name)
                    break

        # Reiterates through multi-variables and adds the first non-None to var-map
        for key, value in final_partial.items():
            full_key = f"{current_prefix}.{key}" if current_prefix else key
            templ_val = templ_partial.get(key, None)
            if isinstance(templ_val, str) and templ_val.startswith("$") or isinstance(templ_val, int):
                full_key = f"{current_prefix}.[{key}]" if current_prefix and "." in key else full_key
                vars = parse_variable(templ_val)
                first_var = None
                for template_var_name in vars:
                    # print(template_var_name, value)
                    if template_var_name in variables:
                        first_var = template_var_name
                        break
                if first_var not in Data["var_map"]:
                    Data["var_map"][first_var] = {full_key: value}
                elif full_key not in Data["var_map"][first_var]:
                    Data["var_map"][first_var][full_key] = value
                else:
                    raise ValueError(f"Key {full_key} already exists in var_map")

    extract(template, final_theme, Data, prefix)

    # Iterates through overrides and add variables that were found later in the process
    trash_bin = {}
    for key, (value, var_name) in not_done.items():
        if var_name in variables:
            if value in variables[var_name]:
                variables[var_name][value] += 1
            else:
                overrides[key] = value
        else:
            trash_bin[key] = (value, var_name)


    # Iterates through trash to find variables not in template that are in final_theme
    # and adds them to overrides
    for key, (value, var_name) in trash_bin.items():
        if key in full_name_map:
            (real_val, templ_val) = full_name_map[key]
            if real_val is None: continue
            elif isinstance(real_val, dict):
                # print("KEY", key, real_val, templ_val)
                for k, v in real_val.items():
                    if v is None: continue
                    if "." in k: k = f"[{k}]"
                    full_name = f"{key}.{k}"
                    # print(full_name, k, v)
                    overrides[full_name] = v
                # print("-"*180)
                continue
            elif isinstance(real_val, list):
                for i, item in enumerate(real_val):
                    if item is None: continue
                    full_name = f"{key}.{i}"
                    overrides[full_name] = item
            else:
                # print(key, real_val, templ_val)
                if templ_val is not None: continue
                overrides[key] = real_val

    return name, Data

def is_hex_color(color):
    if not isinstance(color, str):
        return False
    color_regex = r'(#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3}))'
    return re.match(color_regex, color)


def get_base_color(hex):
    assert hex.startswith("#"), f"Hex color {hex}, must start with #"
    assert len(hex) in (4, 7, 9), f"Hex color {hex}, must be 4, 7, or 9 characters long"
    hex = hex.upper()
    if len(hex) == 4:
        hex = f"#{hex[1]*2}{hex[2]*2}{hex[3]*2}"
    return hex[:7]

def get_alpha(hex):
    assert is_hex_color(hex), "Color must be a hex color"
    if not len(hex) == 9:
        return None
    return hex[7:]


def handle_colors(variables, overrides, var_map, threshold=1):
    """
    Sorts the colors in the variables dictionary and replaces the color with a color variable
    """
    colors = Counter()

    # Builds a list of colors and their counts
    for _, value in variables.items():
        for color, count in value.items():
            if is_hex_color(color):
                color = get_base_color(color)
                colors.update({color: count})

    for _, value in overrides.items():
        if value and is_hex_color(value):
            color = get_base_color(value)
            colors.update({color: 1})

    # Gets the most occuring color for each variable, then replace the color with the color variable
    sorted_variables = { var_name: max(value, key=value.get) for var_name, value in variables.items() }

    # Adds colors discluded in sorted_variables to the overrides
    overrides = cleanup_variables(var_map, sorted_variables, overrides)
    for _, value in overrides.items():
        if value and is_hex_color(value):
            color = get_base_color(value)
            colors.update({color: 1})


    # Creates a color map, color: color_var_name
    colors = [ color[0] for color in sorted(filter(lambda x: x[1] > threshold, colors.items()), key=lambda x: x[1], reverse=True)]
    colors = { color: f"$colors.color{index + 1}" for index, color in enumerate(colors)}

    # Replaces the color with the color variable
    sorted_variables = { var_name: colors.get(get_base_color(value), value) if is_hex_color(value) else value for var_name, value in sorted_variables.items() }
    # sorted_overrides = {
    #     key: colors.get(get_base_color(value), value)
    #     if value and is_hex_color(value)
    #     else value
    #     print("overrides", overrides)
    #     for key, value in overrides.items()
    # }
    sorted_overrides = {}
    for key, value in overrides.items():
        if value and is_hex_color(value):
            color = colors.get(get_base_color(value), value)
            alpha = get_alpha(value)
            can_add_alpha = (color and color.startswith("$") and ".." not in color ) or (color and not color.startswith("$") and len(color) < 9)
            sorted_overrides[key] = color + f"{".." if color.startswith("$") else ""}{alpha}" if alpha and can_add_alpha else color
        else:
            sorted_overrides[key] = value

    # print("overrides", sorted_overrides)

    return colors, sorted_variables, sorted_overrides


def cleanup_variables(var_map, variables, overrides):
    """
    Adds the variables in the var map whose color doesn't match the color in the variables to the overrides
    """
    for var_name, map in var_map.items():
        base_color_groups = {}
        for key, value in map.items():
            if isinstance(value, list):
                listified = listify(value)
                if listified == variables[var_name]:
                    value = listified
                else:
                    raise ValueError(f"Listified value {listified} does not match variable value {variables[var_name]}")
            color = get_base_color(value) if is_hex_color(value) else value
            templ_color = variables.get(var_name, None)
            templ_color = get_base_color(templ_color) if is_hex_color(templ_color) else templ_color

            if color != templ_color:
                alpha = get_alpha(value) if is_hex_color(color) else None
                overrides[key] = color + f"{".." if color.startswith("$") else ""}{alpha}" if alpha else color
            else:
                # Group by base color
                if color not in base_color_groups:
                    base_color_groups[color] = []
                base_color_groups[color].append((key, value))
        # Case where colors have same base color but different alpha
        for base_color, color_group in base_color_groups.items():
            if len(color_group) > 1:
                # Check if there are different alpha values
                alphas = list(get_alpha(color[1]) for color in color_group if is_hex_color(color[1]))
                if len(set(alphas)) > 1:
                    # Set the var_map to remove everything but the most common alpha
                    most_common_alpha = max(alphas, key=lambda x: alphas.count(x))
                    for key, value in color_group:
                        alpha = get_alpha(value)
                        if alpha != most_common_alpha:
                            overrides[key] = value
                            var_map[var_name].pop(key)


    return overrides


def generate_toml(variables, overrides, color_map, var_map, output_path, delete_keys):
    toml_content = ""
    sub_section_regex = r'(.+)\.([^.]+)$'
    sub_sections = {}
    s = lambda x: f"\n[{x}]\n"
    def v(x, y):
        if y == None:
            y = 'false'
        elif isinstance(y, str):
            y = f'"{y}"'
        elif isinstance(y, list):
            y = f'[{", ".join([f"\n\t\"{v}\"" for v in y])}\n]'
        return f"{x} = {f'{y}'}\n"

    def get_original_color(var_name, value):
        if not isinstance(value, str): return value
        val_map = var_map.get("$" + var_name, {})
        colors = set(filter(lambda c: is_hex_color(c) and color_map.get(get_base_color(c)) == value, [val for val in val_map.values()]))
        if len(colors) != 1: return value
        alpha = get_alpha(colors.pop())
        return value + (f"{".." if value.startswith("$") else ""}{alpha}" if alpha else "")

    # Adds the variables to the toml content
    for var_name, value in variables.items():
        # Gets the sub section of the variable
        match = re.match(sub_section_regex, var_name)
        var_name = var_name[1:]
        if not match:
            value = get_original_color(var_name, value)
            if is_listified(value):
                value = delistify(value)
                value = [color_map.get(get_base_color(color), color) if is_hex_color(color) else color for color in value]

            toml_content += v(var_name, value)
        else:
            sub_section, sub_key = match.groups()
            sub_sections[sub_section] = sub_sections.get(sub_section, {})
            sub_sections[sub_section][sub_key] = value

    # Adds the color map to the toml content
    toml_content += s("colors")
    for color, color_var_name in color_map.items():
        color_var_name = color_var_name[8:]
        toml_content += v(color_var_name, color)

    sub_sections = dict(sorted(sub_sections.items()))
    for sub_section, sub_section_values in sub_sections.items():
        sub_section = sub_section[1:]
        toml_content += s(sub_section)
        for sub_key, value in sub_section_values.items():
            value = get_original_color(sub_section + "." + sub_key, value)
            toml_content += v(sub_key, value)

    # Adds the overrides to the toml content
    if overrides:
        toml_content += s("overrides")
    for key, value in overrides.items():
        value = get_original_color(key, value)
        # key = key.replace("style.","").replace("syntax.","")
        toml_content += v(f"\"{key}\"", value)

    # Adds the delete keys to the toml content as a list
    if delete_keys:
        toml_content += s("deletions")
        toml_content += v("keys", delete_keys)

    with open(output_path, 'w') as f:
        f.write(toml_content)

def main():
    parser = argparse.ArgumentParser(
        description="Process JSON template with TOML variables or reverse the process"
    )
    parser.add_argument("template_path", help="Path to the JSON template file")
    parser.add_argument(
        "-o", "--output", help="Output path for the processed JSON file"
    )
    parser.add_argument(
        "-c",
        "--current-dir",
        action="store_true",
        help="Export to the current directory of the template",
    )
    parser.add_argument(
        "-d",
        "--delete",
        action="store_true",
        help="Delete the processed JSON file"
    )
    parser.add_argument(
        "-r", "--reverse",
        help="Reverse the process: extract variables from final theme to create TOML. Provide path to final JSON theme."
    )
    parser.add_argument(
        "-p", "--print",
        action="store_true",
        help="Prints help messages"
    )
    parser.add_argument(
        "-t", "--threshold",
        type=int,
        default=2,
        help="Threshold for color detection"
    )
    parser.add_argument(
        "-n", "--name",
        help="Name of the processed theme"
    )

    args = parser.parse_args()

    template_path = Path(args.template_path)
    template_name = template_path.stem.split(".")[0]

    if args.print:
        global PRINT
        PRINT = True

    if args.delete:
        file_path = Path.home() / ".config" / "zed" / "themes" / f"{template_name}.json"
        if os.path.exists(file_path):
            os.remove(file_path)
            print(f"Deleted processed themes: {file_path}")
            return

    if args.reverse:
        threshold = args.threshold if args.threshold and args.threshold > 0 else 1
        final_theme_path = Path(args.reverse)
        if not final_theme_path.exists():
            print(f"Error: Final theme file not found: {final_theme_path}")
            sys.exit(1)

        output_paths = reverse_process(template_path, final_theme_path, threshold)
        print(f"Extracted variables saved to:")
        for output_path in output_paths:
            print(f"  {output_path}")
    else:
        if args.current_dir:
            output_path = template_path.parent / f"{args.name or template_name}.json"
        elif args.output:
            output_path = Path(args.output)
        elif args.name:
            output_path = Path.home() / ".config" / "zed" / "themes" / f"{args.name}.json"
        else:
            output_path = Path.home() / ".config" / "zed" / "themes" / f"{template_name}.json"

        # Scan for all TOML files in the template directory
        toml_files = list(template_path.parent.glob("*.toml"))

        if not toml_files:
            print(f"Error: No TOML files found in {template_path.parent}")
            sys.exit(1)

        processed_json = process_json_template(template_path, toml_files)

        # Ensure the output directory exists
        os.makedirs(output_path.parent, exist_ok=True)

        with open(output_path, "w") as file:
            json.dump(processed_json, file, indent=2)

        print(f"Processed themes saved to: {output_path}")

if __name__ == "__main__":
    main()
