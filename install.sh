#!/bin/sh
set -e

REPO="dhimasardinata/caxe"
INSTALL_DIR="$HOME/.cx/bin"
BIN_NAME="cx"

echo "Installing caxe (cx)..."

# 1. Detect OS & Arch
OS="$(uname -s)"

case "$OS" in
    Linux)
        OS_TYPE="linux"
        ;;
    Darwin)
        OS_TYPE="macos"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

# 2. Create Directory
mkdir -p "$INSTALL_DIR"

# 3. Fetch Latest Release
echo "Fetching latest version..."
LATEST_JSON=$(curl -s "https://api.github.com/repos/$REPO/releases/latest")
TAG_NAME=$(echo "$LATEST_JSON" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

echo "Latest version: $TAG_NAME"

# 4. Construct Download URL
DOWNLOAD_URL=$(echo "$LATEST_JSON" | grep "browser_download_url" | grep -i "$OS_TYPE" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find a binary for $OS_TYPE in the latest release."
    exit 1
fi

echo "Downloading from $DOWNLOAD_URL..."
curl -L -o "$INSTALL_DIR/$BIN_NAME" "$DOWNLOAD_URL"

chmod +x "$INSTALL_DIR/$BIN_NAME"

# 5. Update Shell Path
SHELL_CONFIG=""
case "$SHELL" in
    */zsh) SHELL_CONFIG="$HOME/.zshrc" ;;
    */bash) SHELL_CONFIG="$HOME/.bashrc" ;;
    *) SHELL_CONFIG="$HOME/.profile" ;; # Fallback
esac

if echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "$INSTALL_DIR is already in PATH."
else
    echo "Adding to PATH in $SHELL_CONFIG..."
    echo "" >> "$SHELL_CONFIG"
    echo "# Caxe Package Manager" >> "$SHELL_CONFIG"
    echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_CONFIG"
    echo "Please run: source $SHELL_CONFIG"
fi

echo "Success! caxe ($TAG_NAME) installed."
