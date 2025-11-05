I want to create a Rust app called `maram` to manage git worktree workflow.

# Terms

- base: the main tree/branch where the final changes are going to be made
- variant branch: a branch/worktree whose changes can be merged onto the base branch
- Worktree set: base + variant trees

### `maram create` alias `maram c`

This should prompt the user for a branch name.
The user will then get an interactive list where they get to add and remove variant names.

The app should then create the branch with the branch name the use specified and also create a branch for each of the specified variants.
Variant branches will be prefixed with `<variant-name>/`. For example if my branch is called `foo` the branch for the variant `bar` will be called `bar/foo`
If no variants specified then only create the base branch. These branches will be branched off the current head.

The base branch should be mounted as a worktree in `$MARAM_DIR/<repo name>/<branch name>/base`.
variant branches should then be mounted as worktrees in `$MARAM_DIR/<repo name>/<branch name>/<variant name>`.
If `$MARAM_DIR` doesn't exist default to `~/maram`.
Get the repo name from remote, if there is no remote use the root directory's name.

Then create a zellij session with the session name `<repo name>-<branch name>`, the session should have a tab for each of the branches including the base one. The tab should be open to the dir of the specified branch.

### `maram checkout <optional branch name>` alias `maram co`

Should list the worktree sets managed by maram for the current repo (i.e. the directories in `$MARAM_DIR/<repo name>/`) and let the user select between them.
Once the user selects the worktree set they want they should attach to the appropriate zellij session, if the session was killed create a new one.
User should also be able to directly access the session by providing the optional argument.

### `maram delete <optional branch name>` alias `maram d`

Should list the worktree sets managed by maram for the current repo (i.e. the directories in `$MARAM_DIR/<repo name>/`) and let the user select between them.
Once the user selects the worktree set they want unmount all the worktrees, kill and delete the zellij session, and delete the directory recreated for the worktrees. Delete all but the base branch from the repo.
User should also be able to directly delete the session by providing the optional argument.

## If these following commands are called outside the worktree set then the command should fail and warn the user

### `maram status`

Should print the current worktree set name, the number of trees in the set and the currently picked variant.

### `maram pick <variant name>` alias `maram p`

Squash and cherry-pick the changes from the variant branch into the base branch.
If the user picks a different variant afterwards discard the changes made by the previous pick and all subsequent changes before applying the new pick.
Make the user aware that this is destructive and if there are conflicts during a cherry-pick let the user handle them.

Here's an example, foo and bar are variants

Initial state:

| base     | foo      | bar      |
| -------- | -------- | -------- |
| commit 1 | commit a | commit x |
|          | commit b |          |

After running `maram pick foo`

| base     | foo      | bar      |
| -------- | -------- | -------- |
| commit 1 | commit a | commit x |
| commit a |          |          |

_commit b is squashed into commit a_
User makes more changes:

| base     | foo      | bar      |
| -------- | -------- | -------- |
| commit 1 | commit a | commit x |
| commit a |          |          |
| commit 2 |          |          |

After running `maram pick bar`

| base     | foo      | bar      |
| -------- | -------- | -------- |
| commit 1 | commit a | commit x |
| commit x |          |          |

### `maram reset`

Reset the base branch to how it was before any picks.

### `maram diff <variant name> <optional second variant name>`

Compare the 2 variants given. The second value will default to the base branch

## General

allow the users to configure a default list of variant at `~/.config/maram/config.toml`
store metadata about the worktree set (e.g. current picked variant) in `$MARAM_DIR/<repo>/<branch>/.maram/`
implement the zellij functionality using layouts, use a kdl library to handle this. If you think it's beneficial to have a permanent copy of the layout but it in `$MARAM_DIR/<repo>/<branch>/.maram/`
