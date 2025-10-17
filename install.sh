#!/bin/bash
# Installation script for ffmpeg-encoder on Linux and macOS
# This script installs the binary to ~/.local/bin and config to ~/.config/ffmpeg-encoder

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Detect OS
OS="$(uname -s)"
case "${OS}" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    *)          MACHINE="UNKNOWN:${OS}"
esac

echo "Detected OS: ${MACHINE}"

# Set installation directories
BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/ffmpeg-encoder"
BINARY_NAME="ffmpeg-encoder"

# Check if binary exists
BINARY_SOURCE="./target/release/${BINARY_NAME}"
if [ ! -f "${BINARY_SOURCE}" ]; then
    echo -e "${RED}Error: Binary not found at ${BINARY_SOURCE}${NC}"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

# Create directories if they don't exist
echo "Creating installation directories..."
mkdir -p "${BIN_DIR}"
mkdir -p "${CONFIG_DIR}"

# Install binary
echo "Installing binary to ${BIN_DIR}..."
cp "${BINARY_SOURCE}" "${BIN_DIR}/${BINARY_NAME}"
chmod +x "${BIN_DIR}/${BINARY_NAME}"

# Install default config if user doesn't have one
if [ ! -f "${CONFIG_DIR}/config.yaml" ]; then
    if [ -f "./config/config.default.yaml" ]; then
        echo "Installing default config to ${CONFIG_DIR}..."
        cp "./config/config.default.yaml" "${CONFIG_DIR}/config.yaml"
        echo -e "${YELLOW}Note: A default config has been created at ${CONFIG_DIR}/config.yaml${NC}"
        echo -e "${YELLOW}Please edit this file to configure your encoding settings.${NC}"
    else
        echo -e "${YELLOW}Warning: No default config found. You'll need to create ${CONFIG_DIR}/config.yaml manually.${NC}"
    fi
else
    echo -e "${GREEN}Existing config found at ${CONFIG_DIR}/config.yaml - keeping it.${NC}"
fi

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo ""
    echo -e "${YELLOW}Warning: $HOME/.local/bin is not in your PATH${NC}"
    echo "Please add the following line to your shell configuration file:"
    echo ""

    if [ "${MACHINE}" = "Mac" ]; then
        if [ -n "$ZSH_VERSION" ]; then
            echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
            echo "  source ~/.zshrc"
        else
            echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bash_profile"
            echo "  source ~/.bash_profile"
        fi
    else
        echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
        echo "  source ~/.bashrc"
    fi
    echo ""
fi

echo ""
echo -e "${GREEN}Installation completed successfully!${NC}"
echo ""
echo "Binary installed to: ${BIN_DIR}/${BINARY_NAME}"
echo "Config directory: ${CONFIG_DIR}"
echo ""
echo "You can now run: ${BINARY_NAME} --help"
echo ""
echo "To uninstall, run: ./uninstall.sh"
