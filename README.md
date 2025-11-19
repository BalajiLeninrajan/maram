# மரம் maram

A CLI tool for managing git worktree workflows with variant branches and Zellij integration.

## Overview

`maram` helps you create and manage multiple git worktrees for a branch, allowing you to work on different variants simultaneously. It automatically integrates with Zellij to create organized terminal sessions with tabs for each worktree.

## Worktree sets

A **worktree set** is a collection of git worktrees organized around a base branch, allowing you to work on multiple variants of the same feature simultaneously.

### Structure

A worktree set consists of:

1. **Base branch/worktree** - The main branch (e.g., `feature-branch`)
   - Located at `~/maram/<repo-name>/<branch-name>/base/`

2. **Variant branches/worktrees** - Separate branches for different approaches
   - Each variant gets its own branch named `<variant>/<base-branch>` (e.g., `approach-a/feature-branch`)
   - Each variant has its own worktree directory at `~/maram/<repo-name>/<branch-name>/<variant-name>/`

### Default Directory Layout

```
~/maram/
└── <repo-name>/
    └── <branch-name>/
        ├── base/              # Base branch worktree
        ├── variant1/          # First variant worktree
        └── variant2/          # Second variant worktree
```

### Features

- **Experiment with different approaches** without losing your work
- **Compare implementations** side-by-side
- **Cherry-pick changes** from variants into the base branch
- **Work on multiple variants** in parallel using Zellij

## Requirements

- **Git** - Required for worktree management
- **Zellij** - Required for session management. Install from [zellij.dev](https://zellij.dev) or via your package manager.

## Installation

### Building from Source

```bash
cargo build --release
```

## Configuration

`maram` uses a TOML configuration file located at `~/.config/maram/config.toml`. The configuration file is automatically created with default values the first time you run `maram`.

### Example Config File

```toml
# Default variants to create when creating a new worktree set
default_variants = ["approach-a", "approach-b"]

# Whether to skip Zellij session attachment by default
# Can be overridden with --no-session flag (acts like a toggle)
no_session = false

# Custom directory for storing worktree sets
# Defaults to ~/maram if not specified
# Supports ~ expansion (e.g., "~/maram" or "/custom/path/maram")
maram_dir = "~/maram"

# Custom Zellij layout template to prefix each tab
# This template is prepended to each tab in the Zellij session
# Useful for adding status bars, tab bars, or other UI elements
prefix_zellij_layout = """
default_tab_template {
  pane size=1 borderless=true {
      plugin location="zellij:tab-bar"
  }
  children
  pane size=2 borderless=true {
      plugin location="zellij:status-bar"
  }
}
"""
```

### Configuration Options

- **`default_variants`** (array of strings, default: `[]`)
  - List of variant names to automatically create when creating a new worktree set
  - If empty, `maram` will prompt you interactively to select variants
  - Example: `default_variants = ["approach-a", "approach-b", "experimental"]`

- **`no_session`** (boolean, default: `false`)
  - If `true`, `maram` will not attach to Zellij sessions by default
  - Can be overridden per-command using the `--no-session` flag
  - Useful if you prefer to manage Zellij sessions manually

- **`maram_dir`** (string, optional, default: `~/maram`)
  - Custom directory path where worktree sets are stored
  - Supports `~` expansion for home directory
  - Can be an absolute path (e.g., `/custom/path/maram`) or relative to home (e.g., `~/maram`)
  - If not specified, defaults to `~/maram`

- **`prefix_zellij_layout`** (string, optional)
  - Custom Zellij layout template that gets prepended to each tab
  - Useful for adding UI elements like tab bars, status bars, or custom panes
  - The template is indented and inserted before the tab content
  - If not specified, tabs are created without any prefix layout

### Config File Location

The configuration file is located at:

- **Linux/macOS**: `~/.config/maram/config.toml`
- **Windows**: `%APPDATA%\maram\config.toml`

If the config file doesn't exist, `maram` will automatically create it with default values on first run.

## Usage

### Create a worktree set

```bash
maram create [branch-name]
# or
maram c [branch-name]
```

Creates a new worktree set with a base branch and optional variant branches. Each variant gets its own worktree directory and branch.

### Checkout a worktree set

```bash
maram checkout [branch-name]
# or
maram co [branch-name]
```

Switches to an existing worktree set and attaches to the Zellij session (or creates one if it doesn't exist).

### Delete a worktree set

```bash
maram delete [branch-name]
# or
maram d [branch-name]
```

Removes a worktree set and all its variant worktrees, while keeping the base branch.

### Status

```bash
maram status
# or
maram s
```

Shows information about the current worktree set.

### Pick a variant

```bash
maram pick [variant-name]
# or
maram p [variant-name]
```

Cherry-picks commits from a variant branch into the base branch.

### Reset

```bash
maram reset
# or
maram r
```

Resets the base branch to its original state, discarding any picked changes.

### Diff

```bash
maram diff <variant1> [variant2]
```

Shows the diff between two variants (or between a variant and base if variant2 is omitted).

### Add a variant

```bash
maram add [variant-name]
# or
maram a [variant-name]
```

Adds a new worktree variant to the current worktree set.

### Remove a variant

```bash
maram remove [variant-name]
# or
maram rm [variant-name]
```

Removes a worktree variant from the current worktree set.

### Sync with parent

```bash
maram sync [branch-name]
# or
maram l [branch-name]
```

Rebases the branches in the current worktree set with the given branch.
By default if no arguments are passed the parent branch is used.

## Examples

```bash
# Create a worktree set with interactive variant selection
maram create feature-branch

# Create with specific variants
maram create feature-branch --variants variant1 variant2

# Checkout without Zellij session
maram checkout feature-branch --no-session

# Pick changes from a variant
maram pick variant1

# Add a new variant to existing worktree set
maram add new-variant

# Remove a variant from worktree set
maram remove old-variant

# Show status
maram status

# Reset base branch
maram reset
```

## TODO

- add support for other multiplexers
