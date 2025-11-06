# மரம் maram

A CLI tool for managing git worktree workflows with variant branches and Zellij integration.

## Overview

`maram` helps you create and manage multiple git worktrees for a branch, allowing you to work on different variants simultaneously. It automatically integrates with Zellij to create organized terminal sessions with tabs for each worktree.

## Installation

```bash
cargo build --release
```

## Usage

### Create a worktree set

```bash
maram create [branch-name]
```

Creates a new worktree set with a base branch and optional variant branches. Each variant gets its own worktree directory and branch.

### Checkout a worktree set

```bash
maram checkout [branch-name]
```

Switches to an existing worktree set and attaches to the Zellij session (or creates one if it doesn't exist).

### Delete a worktree set

```bash
maram delete [branch-name]
```

Removes a worktree set and all its variant worktrees, while keeping the base branch.

### Status

```bash
maram status
```

Shows information about the current worktree set.

### Pick a variant

```bash
maram pick [variant-name]
```

Cherry-picks commits from a variant branch into the base branch.

### Reset

```bash
maram reset
```

Resets the base branch to its original state, discarding any picked changes.

### Diff

```bash
maram diff <variant1> [variant2]
```

Shows the diff between two variants (or between a variant and base if variant2 is omitted).

## Options

- `--no-session` / `-n`: Skip Zellij session creation and drop into the worktree directory instead

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
```
