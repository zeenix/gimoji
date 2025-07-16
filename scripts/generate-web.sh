#!/bin/bash

# Script to generate the gimoji website locally for testing
# Usage: ./scripts/generate-web.sh

set -e

echo "Generating gimoji website..."

# Get the directory where the script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Create website directory
mkdir -p "$PROJECT_ROOT/website"
cp "$PROJECT_ROOT/emojis.json" "$PROJECT_ROOT/website/"

# Copy template files to website directory
cp "$SCRIPT_DIR/index.html.template" "$PROJECT_ROOT/website/index.html"
cp "$SCRIPT_DIR/styles.css.template" "$PROJECT_ROOT/website/styles.css"

echo "✅ Website generated in ./website/"
echo "📁 Files created:"
echo "   - website/index.html"
echo "   - website/styles.css"
echo "   - website/emojis.json"
echo ""
echo "🌐 To test locally:"
echo "   cd website && python3 -m http.server 8000"
echo "   Then open http://localhost:8000"