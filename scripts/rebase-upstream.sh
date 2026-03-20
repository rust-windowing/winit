#!/usr/bin/env bash
set -euo pipefail

UPSTREAM_URL="https://github.com/rust-windowing/winit.git"
BRANCH="remove-private-api"

# Add upstream remote if missing
if ! git remote get-url upstream &>/dev/null; then
    echo "Adding upstream remote..."
    git remote add upstream "$UPSTREAM_URL"
fi

echo "Fetching upstream..."
git fetch upstream

echo "Rebasing $BRANCH onto upstream/master..."
git checkout "$BRANCH"
if git rebase upstream/master; then
    echo "Rebase successful."
    git log --oneline -5
else
    echo ""
    echo "Rebase has conflicts. Resolve them, then run:"
    echo "  git add <resolved files>"
    echo "  git rebase --continue"
    echo ""
    echo "Or abort with: git rebase --abort"
    exit 1
fi
