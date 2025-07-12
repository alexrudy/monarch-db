#!/bin/bash
set -e

echo "Setting up pre-commit hooks for Monarch-DB..."

# Check if pre-commit is installed
if ! command -v pre-commit &> /dev/null; then
    echo "pre-commit not found. Installing via pip..."

    # Try to install with pip3 first, then pip
    if command -v pip3 &> /dev/null; then
        pip3 install pre-commit
    elif command -v pip &> /dev/null; then
        pip install pre-commit
    else
        echo "Error: pip not found. Please install Python and pip first."
        echo "You can install pre-commit with:"
        echo "  pip install pre-commit"
        echo "  # or"
        echo "  pipx install pre-commit"
        exit 1
    fi
fi

# Install the git hook scripts
echo "Installing pre-commit git hooks..."
pre-commit install

# Install commit-msg hook for conventional commits (optional)
pre-commit install --hook-type commit-msg

# Run pre-commit on all files to check setup
echo "Running pre-commit on all files to verify setup..."
if pre-commit run --all-files; then
    echo "✅ Pre-commit setup completed successfully!"
else
    echo "⚠️  Pre-commit found some issues. These have been automatically fixed where possible."
    echo "Please review the changes and commit them."
fi

echo ""
echo "Pre-commit is now configured! It will run automatically on git commit."
echo ""
echo "Useful commands:"
echo "  pre-commit run --all-files                        # Run on all files"
echo "  pre-commit run <hook-id>                          # Run specific hook"
echo "  pre-commit run --hook-stage manual cargo-audit    # Run security audit"
echo "  pre-commit run --hook-stage manual cargo-deny     # Run dependency analysis"
echo "  pre-commit autoupdate                             # Update hook versions"
echo "  pre-commit uninstall                              # Remove hooks"
