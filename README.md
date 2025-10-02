# Theme Generation Tool

A powerful Rust-based CLI tool for generating, reverse-engineering, and managing color themes across multiple platforms and editors. Create consistent themes for Zed and more using a template-based variable substitution system.

## 🚀 Features

- **🎨 Multi-Platform Support**: Generate themes for Zed, VsCode, Textmate, and more (potentially)
- **🔄 Bidirectional Workflow**: Generate themes from templates or reverse-engineer existing themes
- **⚡ Live Development**: Watch mode for real-time theme updates during development
- **🎯 Template System**: Flexible variable substitution with TOML configuration files
- **🔍 Theme Comparison**: Built-in tools to compare and validate theme consistency
- **📦 Project Scaffolding**: Quickly create new theme projects with templates

## 📦 Installation

### From Source
```bash
git clone https://github.com/blaqat/theme-generation.git
cd theme-generation
cargo build --release
# Binary will be available at ./target/release/theme-generation
```

### Usage
```bash
# Add to your PATH or use directly
./target/release/theme-generation --help
```

## 🛠️ Commands

### `generate` - Create Themes from Templates
Generate new theme files by substituting variables in templates.

```bash
theme-generation gen <template_file> <variable_file> [OPTIONS]
```

**Options:**
- `-o <directory>` - Set output directory
- `-i <directory>` - Set input directory for TOML files
- `-p <path>` - JSON path to start generation
- `-n <name>` - Output file name
- `-r` - Overwrite existing files

**Example:**
```bash
theme-generation gen templates/full.json.template my-theme.toml -o themes/
```

### `reverse` - Extract Variables from Existing Themes
Reverse-engineer existing themes to create variable files.

```bash
theme-generation rev <template_file> <original_theme> [OPTIONS]
```

**Options:**
- `-t <int>` - Color threshold for grouping
- `-o <directory>` - Output directory
- `-n <name>` - Output file name
- `-p <path>` - JSON path to start reverse process

**Example:**
```bash
theme-generation rev templates/full.json.template existing-theme.json -o variables/
```

### `check` - Compare Themes
Validate theme consistency and compare generated vs original themes.

```bash
theme-generation check <original_file> <generated_file>
```

### `watch` - Live Development Mode
Monitor files for changes and automatically regenerate themes.

```bash
theme-generation watch <template_file> <variable_file|all> [OPTIONS]
```

**Options:**
- `-p <path>` - Inner theme path
- `-o <directory>` - Output directory
- `-n <name>` - Output theme name
- `-i <directory>` - TOML files directory

**Example:**
```bash
theme-generation watch templates/full.json.template all -o build/
```

### `edit` - Streamlined Theme Development
Combines reverse engineering and watch mode for rapid theme development. Automatically reverse-engineers an existing theme to extract variables, then starts watch mode for live editing.

```bash
theme-generation edit <template_file> <original_theme> [OPTIONS]
```

**Options:**
- All watch command options (except `-o` for reverse step)
- All reverse command options (except `-o` which uses current directory)

**Example:**
```bash
theme-generation edit templates/full.json.template existing-theme.json -n my-theme
```

**Workflow:**
1. Reverse-engineers the original theme to create variable files in current directory
2. Automatically starts watch mode for live development
3. Perfect for adapting existing themes or rapid prototyping

### `new` - Create New Theme Project
Scaffold a new theme project with templates and examples.

```bash
theme-generation new <project_name>
```

### `help` - Get Command Help
```bash
theme-generation help [command]
```

## 📁 Project Structure

```
theme-generation/
├── src/                    # Rust source code
│   ├── commands/          # Command implementations
│   ├── utils/             # Utility functions
│   └── main.rs           # Entry point
├── templates/             # Theme templates
│   ├── full.json.template    # Complete theme template
│   ├── simple.json.template # Minimal theme template
│   ├── project/             # Project scaffolding templates
│   └── variable/            # Variable file templates
├── playground/            # Testing and examples
│   ├── test-themes/       # Sample themes by platform
│   ├── test-code/         # Code samples for testing
│   └── edit-mode/         # Live editing examples
└── target/               # Build output
```

## 🎨 Theme Development Workflow

### 1. Create Variable File
Define your theme colors and variables in a TOML file:

```toml
name = "My Awesome Theme"
theme = "dark"

[color]
primary = "#5E81AC"
secondary = "#81A1C1"
background = "#2E3440"
foreground = "#D8DEE9"

[button]
active = "#434C5E"
hover = "#4C566A"
```

### 2. Choose a Template
Select from available templates or create your own:
- `full.json.template` - Complete theme with all options
- `simple.json.template` - Minimal theme template
- Custom templates for specific platforms

### 3. Generate Theme
```bash
theme-generation gen templates/full.json.template my-theme.toml -o output/
```

### 4. Test and Iterate
Use watch mode for live development:
```bash
theme-generation watch templates/full.json.template my-theme.toml -o output/
```

### 5. Validate
Compare your generated theme with the original:
```bash
theme-generation check original-theme.json generated-theme.json
```

## 🔧 Template System

### Variable Substitution
Templates use `{{ variable_name }}` syntax for substitution:

```json
{
  "name": "{{ name }}",
  "colors": {
    "primary": "{{ color.primary }}",
    "background": "{{ color.background }}"
  }
}
```

### Advanced Features
- **Nested Variables**: `{{ section.subsection.value }}`
- **Fallbacks**: `{{ primary|fallback_color }}`
- **Color Operations**: Built-in color manipulation functions
- **Conditional Logic**: Template conditionals for different themes

## 🎯 Supported Platforms

- **⚡ Zed Editor**: Original editor this was made for
- **🎭 VS Code**: Full theme support with syntax highlighting
- **📝 TextMate/Sublime**: Classic editor themes
- **🔧 Custom Platforms**: Extensible template system

## 📚 Examples

### Generate Theme
```bash
theme-generation gen templates/vscode.json.template nord-theme.toml -o vscode-themes/
```

### Reverse Existing Theme
```bash
theme-generation rev templates/vscode.json.template existing-vscode-theme.json -o variables/
```

### Live Development
```bash
theme-generation edit templates/full.json.template my-theme.toml -o build/ -n my-theme
```

## 🧪 Testing

The `playground/` directory contains comprehensive testing resources:
- Sample themes for different platforms
- Test code files in various languages
- Comparison tools for validation

```bash
# Test your theme against sample code
cd playground/test-code/
# Open generated themes in your editor to preview
```