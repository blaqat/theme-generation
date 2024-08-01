import toml
import json
import re
import sys
import os
from pathlib import Path
import argparse

# Updated patterns to match list variables
FILE_PATTERN = r'"(\$[a-zA-Z0-9._]+(?:\s*,\s*\$[a-zA-Z0-9._]+)*)"'
REC_PATTERN = r'(\$[a-zA-Z0-9._]+(?:\s*,\s*\$[a-zA-Z0-9._]+)*)'
ALPHA_PATTERN = r'\.\.([a-zA-Z0-9][a-zA-Z0-9])'

def load_toml(file_path):
    with open(file_path, 'r') as file:
        return toml.load(file)

def resolve_variable(var_name, toml_data, had_alpha=False):
    if not had_alpha:
      a = re.search(ALPHA_PATTERN, var_name)
      alpha = a.group(1) if a else False

      if alpha:
        var_name = var_name.replace(f"..{alpha}", '')
        value = resolve_variable(var_name, toml_data, had_alpha=True)
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

def substitute_variables(text, toml_data, recursive=False):
    def replacer(match):
        var_names = [name.strip() for name in match.group(1).split(',')]

        for var_name in var_names:
            if var_name.startswith('$'):
                var_name = var_name[1:]  # Remove the leading $
            value = resolve_variable(var_name, toml_data)

            if value is None:
                # Try to resolve parent if child not found
                parent = var_name.split('.')[0]
                value = resolve_variable(parent, toml_data)

            if value is None or isinstance(value, bool):
                continue  # Try next variable in the list

            # If value isn't a number or string, skip to next variable
            if not isinstance(value, (str, int, float)):
                continue

            # Recursive substitution if value is a string and contains a variable
            if isinstance(value, str) and '$' in value:
                return substitute_variables(value, toml_data, recursive=True)

            # Return the value as-is (without quotes) if it's not a string
            if not isinstance(value, str):
                return json.dumps(value)

            # For strings, return with quotes
            val = json.dumps(value)

            return val

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
        theme_template = json.dumps(template['themes'][0])
        processed_theme = json.loads(substitute_variables(theme_template, toml_data))

        # Apply overrides
        if 'overrides' in toml_data:
            apply_overrides(processed_theme['style'], toml_data['overrides'])
            apply_overrides(processed_theme['style']['syntax'], toml_data['overrides'])
        if 'overrides-regex' in toml_data:
            apply_regex_overrides(processed_theme['style'], toml_data['overrides-regex'])
            apply_regex_overrides(processed_theme['style']['syntax'], toml_data['overrides-regex'])

        processed_themes.append(processed_theme)

    template['themes'] = processed_themes
    return template

def wildcard_to_regex(pattern: str) -> str:
    # Escape special regex characters in the pattern
    escaped_pattern = re.escape(pattern)
    # Replace wildcards with regex equivalents
    escaped_pattern = escaped_pattern.replace(r'\*\*\*', '.*')
    escaped_pattern = escaped_pattern.replace(r'\.\*\*', r'\..*')
    escaped_pattern = escaped_pattern.replace(r'\*\*\.', r'.*\.')
    escaped_pattern = escaped_pattern.replace(r'\*', r'\w*')
    # Ensure that the pattern matches the entire string
    return f'^{escaped_pattern}$'

def match_wildcard(pattern, key):
    return re.match(wildcard_to_regex(pattern), key)

def find_matching_keys(data, pattern):
    matches = []
    for key in data.keys():
        if match_wildcard(pattern, key):
            matches.append(key)
    return matches

def apply_overrides(theme, overrides):
    for override_key, override_value in overrides.items():
        matching_keys = find_matching_keys(theme, override_key)
        for key in matching_keys:
            if isinstance(override_value, dict):
              theme[key].update(override_value)
            elif type(theme[key]) == type(override_value) or theme[key] == None:
              theme[key] = override_value
            else:
              print(f"Override value type mismatch for key {key}")
    return theme

def apply_regex_overrides(theme, regex_overrides):
    for regex_pattern, override_value in regex_overrides.items():
        for key in theme.keys():
            if re.match(regex_pattern, key):
                theme[key] = override_value
    return theme

def reverse_process(template_path, final_theme_path):
    with open(template_path, 'r') as f:
        template = json.load(f)

    with open(final_theme_path, 'r') as f:
        final_theme = json.load(f)

    variables, overrides = extract_variables(template, final_theme)
    color_counts = count_colors(variables)

    output_toml_path = template_path.with_suffix('.toml')
    generate_toml(variables, color_counts, overrides, output_toml_path)
    return output_toml_path

def extract_variables(template, final_theme):
    variables = {}
    overrides = {}

    def extract_recursive(template_part, final_part, prefix=''):
        if isinstance(template_part, dict):
            for key, value in template_part.items():
                new_prefix = f"{prefix}.{key}" if prefix else key
                extract_recursive(value, final_part.get(key, {}), new_prefix)
        elif isinstance(template_part, str):
            if template_part.startswith('$'):
                var_names = re.findall(r'\$([^,\s]+)', template_part)
                if len(var_names) > 1:
                    # Handle multiple variables
                    for var_name in var_names:
                        if var_name not in variables:
                            variables[var_name] = final_part
                        elif variables[var_name] != final_part:
                            overrides[prefix] = final_part
                else:
                    var_name = var_names[0]
                    if var_name not in variables:
                        variables[var_name] = final_part
                    elif variables[var_name] != final_part:
                        overrides[prefix] = final_part
            elif template_part != final_part:
                overrides[prefix] = final_part

    extract_recursive(template['themes'][0], final_theme['themes'][0])
    return variables, overrides

def count_colors(variables):
    color_counts = {}
    for value in variables.values():
        if isinstance(value, str):
            # Remove alpha channel for counting
            color = re.sub(r'([0-9a-fA-F]{6})[0-9a-fA-F]{2}', r'\1', value)
            if re.match(r'^#[0-9a-fA-F]{6}$', color):
                color_counts[color] = color_counts.get(color, 0) + 1
    return color_counts

def generate_toml(variables, color_counts, overrides, output_path):
    toml_content = ""
    color_map = {}

    # Generate [colors] section
    toml_content += "[colors]\n"
    for i, (color, count) in enumerate(color_counts.items(), 1):
        if count > 1:
            color_name = f"color{i}"
            toml_content += f'{color_name} = "{color}"\n'
            color_map[color] = color_name
    toml_content += "\n"

    # Generate main sections
    main_sections = ['foreground', 'background', 'border', 'tabs', 'cursor', 'scrollbar', 'button', 'element', 'scheme']
    for section in main_sections:
        section_vars = {k: v for k, v in variables.items() if k.startswith(f"{section}.")}
        if section_vars:
            toml_content += f"[{section}]\n"
            for var, value in section_vars.items():
                var_name = var.split('.')[-1]
                toml_content += f'{var_name} = "{value}"\n'
            toml_content += "\n"

    # Generate [syntax] section
    syntax_vars = {k: v for k, v in variables.items() if k.startswith("syntax.")}
    if syntax_vars:
        toml_content += "[syntax]\n"
        for var, value in syntax_vars.items():
            parts = var.split('.')
            if len(parts) > 2:
                subsection = '.'.join(parts[1:-1])
                key = parts[-1]
                toml_content += f'[syntax.{subsection}]\n'
                toml_content += f'{key} = "{value}"\n'
        toml_content += "\n"

    # Generate [overrides] section
    if overrides:
        toml_content += "[overrides]\n"
        for key, value in overrides.items():
            toml_content += f'"{key}" = "{value}"\n'

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

    args = parser.parse_args()

    template_path = Path(args.template_path)
    template_name = template_path.stem.split(".")[0]

    if args.delete:
        file_path = Path.home() / ".config" / "zed" / "themes" / f"{template_name}.json"
        if os.path.exists(file_path):
            os.remove(file_path)
            print(f"Deleted processed themes: {file_path}")
            return

    if args.reverse:
        final_theme_path = Path(args.reverse)
        if not final_theme_path.exists():
            print(f"Error: Final theme file not found: {final_theme_path}")
            sys.exit(1)

        output_toml_path = reverse_process(template_path, final_theme_path)
        print(f"Extracted variables saved to: {output_toml_path}")
    else:
        if args.current_dir:
            output_path = template_path.parent / f"{template_name}.json"
        elif args.output:
            output_path = Path(args.output)
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
