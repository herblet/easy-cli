Script Annotations
---
easy-cli scans your scripts for special annotation comments to build a CLI and shell completion automatically.

# General rules:

- Only comments are parsed: a tag must appear on a line that, after optional indentation, starts with `#`, followed by a space and `@`.
    - Example: `# @about This is my command`
- Tags are case-sensitive and must be written exactly as shown below (all lowercase).
- Unknown tags are ignored.
- Tags before the first `@sub` apply to the top-level command for the script. Each `@sub <name>` starts a new subcommand group; tags that follow it (until the next `@sub`) configure that subcommand.
- If there are no tags in a script, the script still becomes a command named after the filename (without extension) and accepts any arguments (they are passed through to the script).

## Identifier rules:

- Names parsed by `@name`, `@sub`, and `@arg` are identifiers without spaces or `-` (hyphen). Prefer letters, numbers, and underscores.

## Boolean and type parsing:

- Booleans are case-insensitive: `true`/`false`.
- Argument types are written in angle brackets, case-insensitive: `<path>`, `<file>`, `<dir>`. If omitted or unknown, type is `unknown`.

# Supported tags

## `@ignore`

**Syntax:** `# @ignore ...`<br>
**Scope**: Top-level only (before any `@sub`).

Ignore this script entirely (it won’t produce a CLI command).

This must be the first recognized tag in the file to take effect.

It is useful when you have 'helper' scripts in the same directory that you don’t want to expose directly as commands.

## `@name <identifier>`

**Syntax**: `# @name <identifier>`<br>
**Scope**: Top-level only (before any `@sub`).

Sets the command name. If not present, the filename is used as the name.

## `@about <description>`

**Syntax**: `# @about <free text>`
**Scope**: Any command.

A human-readable description shown in help.

## `@opt`

**Syntax**: `# @opt <long-name> ['<s>'] [true|false] [<description...>]`<br>
**Scope**: Top-level and inside subcommands.

- `<long-name>`: long option name (identifier, no spaces or `-`).
- `'<s>'` (optional): a single-character short option written in single quotes, e.g. `'f'`.
- `true|false` (optional): whether the option takes a parameter. Default: `false` (flag).
- `<description...>` (optional): rest of line.

- Examples:
    - `# @opt verbose A verbose flag`
    - `# @opt output true Output file path`
    - `# @opt longname 'l' true The description of longname`

## `@arg <name> [<optional?>] [<type>] [<description>]`

**Syntax**: `# @arg <name> [true|false] [<type>] [<description...>]`<br>
**Scope**: Top-level and inside subcommands. If the top-level command has arguments, subcommands are not allowed.

- `<name>`: identifier (no spaces or `-`)
- `true|false` (optional): whether the argument is optional. Default: `false`.
- `<type>` (optional): one of `<path>`, `<file>`, `<dir>` (angle brackets required). Default: `unknown`.
- `<description...>` (optional): the rest of the line is taken as description.

- Example:
    - `# @arg src <path> Source path`
    - `# @arg mode true Optional mode`
    - `# @arg file true <file> An optional input file`

## `@vararg <name> [<optional?>] [<type>] [<description>]`

**Syntax**: `# @vararg <name> [true|false] [<type>] [<description...>]`<br>
**Scope**: Top-level and inside subcommands, but only one `@vararg` is allowed per command, and it must be the last argument.

Identical as `@arg`, but marks the argument as variadic (can capture multiple values or “the rest”).

- Example:
    - `# @vararg files <file> One or more files`

## `@sub`

**Syntax**: `# @sub <identifier>`<br>

Begins a new subcommand group named `<identifier>`. The tags that follow (until the next `@sub`) define that subcommand’s
description, args, and options.

- Allowed within a subcommand group: `@about`, `@arg`, `@vararg`, `@opt`.

# Complete Example

print.zsh:

```
# @name print
# @about Prints text in various ways
# @opt uppercase 'u' Make the output uppercase
# @opt output true Write output to this file

# @sub quick
# @about Print a fixed greeting
# @opt loud 'l' Shout the greeting
# @arg text <path> The text or file to print
# @vararg rest Additional items to print

# @sub file
# @about Print from a file
# @arg file <file> The file to read
```

# Notes

- Indentation before `#` is allowed.
- Tags are only recognized in comments; lines without `#` are ignored.
- A one-letter word at the start of an option description is fine; it’s not confused with a quoted short name (e.g. `# @opt opt A great option`).
