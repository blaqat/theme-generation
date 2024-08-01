import toml
import json
import re
import sys
import os
from pathlib import Path
import argparse

FILE_PATTERN = r'"\$([a-zA-Z0-9._]+)"'
REC_PATTERN = r'\$([a-zA-Z0-9._]+)'
ALPHA_PATTERN = r'\.\.([a-zA-Z0-9][a-zA-Z0-9])'

def load_toml(file_path):
  with open(file_path, 'r') as file:
      return toml.load(file)

def resolve_variable(var_name, toml_data):
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
      var_name = match.group(1)
      value = resolve_variable(var_name, toml_data)

      if value is None:
          # Try to resolve parent if child not found
          parent = var_name.split('.')[0]
          value = resolve_variable(parent, toml_data)

      if value is None or isinstance(value, bool):
          return 'null'

      # IF value isnt a number or string, return null
      # This is to prevent Object or Array from being returned as a string
      if not isinstance(value, (str, int, float)):
          return 'null'

      # Recursive substitution if value is a string and contains a variable
      if isinstance(value, str) and '$' in value:
          alpha = re.search(ALPHA_PATTERN, value)
          value = value if not alpha else value.replace(f"..{alpha.group(1)}", '')
          sub = substitute_variables(value, toml_data, recursive=True)
          sub = sub[0:-1] + alpha.group(1) + '"' if alpha else sub
          return sub

      # Return the value as-is (without quotes) if it's not a string
      if not isinstance(value, str):
          return json.dumps(value)

      # For strings, return with quotes
      return json.dumps(value)


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
      processed_themes.append(processed_theme)

  template['themes'] = processed_themes
  return template

def main():
    parser = argparse.ArgumentParser(
        description="Process JSON template with TOML variables"
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

    args = parser.parse_args()

    template_path = Path(args.template_path)

    template_name = template_path.stem.split(".")[0]

    if args.delete:
      file_path = Path.home() / ".config" / "zed" / "themes" / f"{template_name}.json"
      if os.path.exists(file_path):
        os.remove(file_path)
        print(f"Deleted processed themes: {file_path}")
        return

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
