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

### After Completing Any Task

**IMPORTANT: After completing a task, always suggest committing and pushing the changes to the remote repository.**

This ensures changes are properly saved and shared with the team.

#### Implementation
When a task is completed:
1. Show the user what files were changed with `git status`
2. Suggest staging the changes with `git add`
3. Suggest committing with a descriptive message
4. Suggest pushing to the remote repository

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
