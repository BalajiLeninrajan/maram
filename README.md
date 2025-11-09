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
        ├── variant2/          # Second variant worktree
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

```bash
cargo build --release
```

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
