#!/bin/bash
# Pull updated code at session start

echo "Pulling latest changes..." >&2

cd "$CLAUDE_PROJECT_DIR" || exit 1

# Pull latest changes from current branch
git pull origin "$(git rev-parse --abbrev-ref HEAD)" 2>&1 || {
    echo "Git pull failed, but continuing..." >&2
}

echo "Repository updated" >&2
exit 0
