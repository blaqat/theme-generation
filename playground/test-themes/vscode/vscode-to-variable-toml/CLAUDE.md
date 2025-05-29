# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview
This is a theme generation tool for converting between VSCode and Zed editor themes using variable templates in TOML format.

## Code Style Guidelines
- TOML variables use pipe (`|`) delimiters for fallback chains (e.g., `syntax.param.color|syntax.var.fg`)
- JSON templates use `$` placeholders that reference TOML variables
- Follow existing variable naming patterns in the TOML files
- Maintain hierarchical structure in variable definitions

## Workflow
- Template-based conversion between VSCode and Zed theme formats
- Variable mappings defined in template-variables.toml
- Theme templates in new-vscode.template.json and zed.template.json

## Reference
- Example VSCode themes in example-vscode-themes/
- Previous implementation examples in previous-attempt-example/
