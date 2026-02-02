# Custom Copilot Instructions

## Git Workflow Rules

### Before Starting Any Task

**CRITICAL: Before starting any new task, always run `git pull` to update the current branch with the latest changes from the remote repository.**

This ensures you're working with the most up-to-date code and prevents merge conflicts.

#### Implementation
When a user requests a task:
1. First, check if you're in a git repository
2. Run `git pull` (or `git pull origin <current-branch>`)
3. Verify the pull succeeded
4. Then proceed with the task

#### Example
```bash
# Before starting any task
git pull && git status
```

### Before Committing Changes

**CRITICAL: Before committing any changes, always run the following validation steps:**

1. **Build the project** - Ensure the code compiles without errors
2. **Run Clippy** - Check for lints and warnings
3. **Run tests** - Run unit tests (excluding module which requires Redis runtime)
4. **Run as Redis module** - Start the module in Redis to verify it loads
5. **Sanity test with redis-cli** - Run basic commands to verify functionality

#### Implementation
Before committing:
```bash
# 1. Build the project
cargo build --release

# 2. Run Clippy for lints
cargo clippy --workspace -- -D warnings

# 3. Run tests (exclude module - it requires Redis allocator)
cargo test --workspace --exclude falkorsemantic-module

# 4. Start Redis with the module (in background)
redis-server --loadmodule ./target/release/libfalkorsemantic_module.so &

# 5. Run sanity test with redis-cli
redis-cli PING
redis-cli MODULE LIST  # Verify module is loaded

# 6. Stop Redis
redis-cli SHUTDOWN NOSAVE
```

**Note:** The `falkorsemantic-module` crate cannot be tested with regular `cargo test` because it requires the Redis allocator. Use `make test` or exclude the module explicitly.

#### Quick Validation Script
```bash
# Full pre-commit validation (use make commands when available)
make build && \
make lint && \
make test && \
echo "Starting Redis with module..." && \
redis-server --loadmodule ./target/release/libfalkorsemantic_module.so --daemonize yes && \
sleep 2 && \
redis-cli PING && \
redis-cli MODULE LIST | grep -i falkor && \
redis-cli SHUTDOWN NOSAVE && \
echo "All checks passed!"
```

Or without make:
```bash
cargo build --release && \
cargo clippy --workspace -- -D warnings && \
cargo test --workspace --exclude falkorsemantic-module && \
echo "Starting Redis with module..." && \
redis-server --loadmodule ./target/release/libfalkorsemantic_module.so --daemonize yes && \
sleep 2 && \
redis-cli PING && \
redis-cli MODULE LIST | grep -i falkor && \
redis-cli SHUTDOWN NOSAVE && \
echo "All checks passed!"
```

### Committing and Creating Pull Requests

**CRITICAL: Never commit directly to main. Always create a feature branch, commit there, and open a Pull Request.**

#### Implementation
When ready to commit changes:

1. **Fetch and rebase with main** - Ensure your changes are on top of latest main
2. **Create a feature branch** - Use descriptive branch names
3. **Commit changes** - With a clear, descriptive message
4. **Push to remote** - Push the feature branch
5. **Create a Pull Request** - Open a PR for review

#### Branch Naming Convention
Use descriptive branch names with prefixes:
- `feat/` - New features (e.g., `feat/json-ld-parser`)
- `fix/` - Bug fixes (e.g., `fix/context-resolution`)
- `docs/` - Documentation changes (e.g., `docs/update-readme`)
- `refactor/` - Code refactoring (e.g., `refactor/parser-cleanup`)

#### Step-by-Step Workflow
```bash
# 1. Ensure main is up to date
git checkout main
git pull origin main

# 2. Create and switch to a feature branch
git checkout -b feat/your-feature-name

# 3. Make your changes, then stage and commit
git add .
git commit -m "feat: descriptive commit message"

# 4. Fetch latest main and rebase to resolve conflicts
git fetch origin main
git rebase origin/main

# 5. If conflicts occur, resolve them:
#    - Edit conflicting files
#    - git add <resolved-files>
#    - git rebase --continue

# 6. Push the feature branch to remote
git push -u origin feat/your-feature-name

# 7. Create a Pull Request using GitHub CLI
gh pr create --title "Your PR Title" --body "Description of changes"
```

#### Handling Rebase Conflicts
When conflicts occur during rebase:
```bash
# 1. Git will pause and show conflicting files
git status  # Shows files with conflicts

# 2. Edit each conflicting file to resolve conflicts
#    Look for <<<<<<< HEAD, =======, and >>>>>>> markers

# 3. After resolving, stage the files
git add <resolved-file>

# 4. Continue the rebase
git rebase --continue

# 5. If rebase gets too complex, you can abort and try again
git rebase --abort
```

#### Creating Pull Request
```bash
# Using GitHub CLI (preferred)
gh pr create --title "feat: add JSON-LD parser" --body "
## Description
Brief description of changes

## Changes
- Change 1
- Change 2

## Testing
- [ ] Build passes
- [ ] Clippy passes
- [ ] Tests pass
- [ ] Redis module loads
- [ ] Sanity tests pass
"

# Or create PR with auto-fill from commits
gh pr create --fill
```

### After Completing Any Task

**IMPORTANT: After completing a task and passing all validation checks, create a feature branch and open a Pull Request.**

#### Complete Workflow Summary
When a task is completed:
1. Run the pre-commit validation steps (build, clippy, test, redis module)
2. Fetch latest main and rebase
3. Resolve any conflicts
4. Create a feature branch (if not already on one)
5. Commit with a descriptive message
6. Push to remote
7. Create a Pull Request

#### Example Complete Flow
```bash
# After completing work - full workflow
git checkout main && git pull origin main
git checkout -b feat/my-feature
git add .
git commit -m "feat: descriptive commit message"
git fetch origin main && git rebase origin/main
git push -u origin feat/my-feature
gh pr create --fill
```

#### Suggested Prompt
After completing work, remind the user:
> "Task complete! I'll now:
> 1. Run validation checks (build, clippy, test, redis module)
> 2. Create a feature branch
> 3. Commit and push changes
> 4. Open a Pull Request
> 
> Would you like me to proceed?"
