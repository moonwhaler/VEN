#!/bin/bash
# Uninstallation script for ffmpeg-encoder on Linux and macOS

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Set installation directories
BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/ffmpeg-encoder"
BINARY_NAME="ffmpeg-encoder"

echo "Uninstalling ffmpeg-encoder..."
echo ""

# Remove binary
if [ -f "${BIN_DIR}/${BINARY_NAME}" ]; then
    echo "Removing binary from ${BIN_DIR}..."
    rm "${BIN_DIR}/${BINARY_NAME}"
    echo -e "${GREEN}Binary removed.${NC}"
else
    echo -e "${YELLOW}Binary not found at ${BIN_DIR}/${BINARY_NAME}${NC}"
fi

# Ask about config directory
if [ -d "${CONFIG_DIR}" ]; then
    echo ""
    echo -e "${YELLOW}Config directory found at ${CONFIG_DIR}${NC}"
    read -p "Do you want to remove the config directory and all settings? (y/N): " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "${CONFIG_DIR}"
        echo -e "${GREEN}Config directory removed.${NC}"
    else
        echo -e "${GREEN}Config directory kept.${NC}"
    fi
else
    echo -e "${YELLOW}Config directory not found at ${CONFIG_DIR}${NC}"
fi

echo ""
echo -e "${GREEN}Uninstallation completed!${NC}"
echo ""
echo "Note: The installation did not modify your shell configuration."
echo "If you manually added ~/.local/bin to your PATH, you may want to remove it."
