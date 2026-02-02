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
3. **Run as Redis module** - Start the module in Redis to verify it loads
4. **Sanity test with redis-cli** - Run basic commands to verify functionality

#### Implementation
Before committing:
```bash
# 1. Build the project
cargo build --release

# 2. Run Clippy for lints
cargo clippy --all-targets --all-features -- -D warnings

# 3. Run tests
cargo test

# 4. Start Redis with the module (in background)
redis-server --loadmodule ./target/release/libfalkorsemantic_module.so &

# 5. Run sanity test with redis-cli
redis-cli PING
redis-cli MODULE LIST  # Verify module is loaded

# 6. Stop Redis
redis-cli SHUTDOWN NOSAVE
```

#### Quick Validation Script
```bash
# Full pre-commit validation
cargo build --release && \
cargo clippy --all-targets -- -D warnings && \
cargo test && \
echo "Starting Redis with module..." && \
redis-server --loadmodule ./target/release/libfalkorsemantic_module.so --daemonize yes && \
sleep 2 && \
redis-cli PING && \
redis-cli MODULE LIST | grep -i falkor && \
redis-cli SHUTDOWN NOSAVE && \
echo "All checks passed!"
```

### After Completing Any Task

**IMPORTANT: After completing a task and passing all validation checks, always suggest committing and pushing the changes to the remote repository.**

This ensures changes are properly saved and shared with the team.

#### Implementation
When a task is completed:
1. Run the pre-commit validation steps above
2. Show the user what files were changed with `git status`
3. Suggest staging the changes with `git add`
4. Suggest committing with a descriptive message
5. Suggest pushing to the remote repository

#### Example
```bash
# After completing a task
git status
git add .
git commit -m "descriptive commit message"
git push
```

#### Suggested Prompt
After completing work, remind the user:
> "Task complete! Would you like me to commit and push these changes? I can help you with:
> - `git add .` to stage all changes
> - `git commit -m 'your message'` to commit
> - `git push` to push to remote"
