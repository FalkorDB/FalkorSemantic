#!/bin/bash
# Build project, run tests, commit changes, and suggest PR creation

cd "$CLAUDE_PROJECT_DIR" || exit 1

echo "=== Starting finalize workflow ===" >&2

# Step 1: Build the project
echo "Building project..." >&2
if ! make build 2>&1; then
    echo "{\"decision\": \"block\", \"reason\": \"Build failed. Please fix build errors before continuing.\"}"
    exit 0
fi
echo "Build successful" >&2

# Step 2: Run sanity tests
echo "Running tests..." >&2
if ! make test 2>&1; then
    echo "{\"decision\": \"block\", \"reason\": \"Tests failed. Please fix test failures before committing.\"}"
    exit 0
fi
echo "Tests passed" >&2

# Step 3: Check for uncommitted changes
echo "Checking for uncommitted changes..." >&2
if git diff-index --quiet HEAD -- 2>/dev/null; then
    echo "No changes to commit" >&2
    echo "{\"systemMessage\": \"Workflow complete. No changes to commit.\"}"
    exit 0
fi

# Step 4: Suggest committing and PR creation
BRANCH=$(git rev-parse --abbrev-ref HEAD)
MAIN_BRANCH="main"

if [ "$BRANCH" != "main" ] && [ "$BRANCH" != "master" ]; then
    echo "{\"systemMessage\": \"Build and tests passed. There are uncommitted changes. Consider: 1) Committing changes with /commit 2) Creating a PR with: gh pr create --base $MAIN_BRANCH --head $BRANCH\"}"
else
    echo "{\"systemMessage\": \"Build and tests passed. There are uncommitted changes on $BRANCH. Consider committing with /commit\"}"
fi

exit 0
