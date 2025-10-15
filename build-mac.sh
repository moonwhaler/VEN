#!/usr/bin/env bash

# FFmpeg Autoencoder - macOS Build Script
# Usage: ./build-mac.sh [production|dev]

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Display banner
echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   FFmpeg Autoencoder Build Script     ║${NC}"
echo -e "${BLUE}║              macOS                     ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust/Cargo is not installed.${NC}"
    echo -e "${YELLOW}Please install Rust from: https://rustup.rs/${NC}"
    echo ""
    echo "Quick install:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Determine build mode
BUILD_MODE="${1:-production}"

case "$BUILD_MODE" in
    production|prod)
        echo -e "${GREEN}Building in PRODUCTION mode...${NC}"
        echo -e "${BLUE}Optimizations: Maximum (opt-level=3, LTO enabled)${NC}"
        echo -e "${BLUE}Target: $(uname -m)-apple-darwin${NC}"
        echo ""

        cargo build --release

        BINARY_PATH="./target/release/ffmpeg-encoder"
        echo ""
        echo -e "${GREEN}✓ Production build completed successfully!${NC}"
        ;;

    dev|development)
        echo -e "${GREEN}Building in DEVELOPMENT mode...${NC}"
        echo -e "${BLUE}Optimizations: Minimal (debug info included)${NC}"
        echo -e "${BLUE}Target: $(uname -m)-apple-darwin${NC}"
        echo ""

        cargo build

        BINARY_PATH="./target/debug/ffmpeg-encoder"
        echo ""
        echo -e "${GREEN}✓ Development build completed successfully!${NC}"
        ;;

    *)
        echo -e "${RED}Error: Invalid build mode '${BUILD_MODE}'${NC}"
        echo -e "${YELLOW}Usage: $0 [production|dev]${NC}"
        echo ""
        echo "Build modes:"
        echo "  production, prod  - Optimized release build (slower compile, faster runtime)"
        echo "  dev, development  - Debug build (faster compile, includes debug symbols)"
        exit 1
        ;;
esac

# Display binary information
if [ -f "$BINARY_PATH" ]; then
    echo ""
    echo -e "${BLUE}Binary location:${NC} $BINARY_PATH"

    # Get file size
    SIZE=$(du -h "$BINARY_PATH" | cut -f1)
    echo -e "${BLUE}Binary size:${NC} $SIZE"

    # Display architecture
    ARCH=$(file "$BINARY_PATH" | grep -o 'arm64\|x86_64' || echo "unknown")
    echo -e "${BLUE}Architecture:${NC} $ARCH"

    echo ""
    echo -e "${GREEN}You can now run the encoder with:${NC}"
    echo -e "  $BINARY_PATH --help"
    echo ""
    echo -e "${YELLOW}To install system-wide (optional):${NC}"
    echo -e "  sudo cp $BINARY_PATH /usr/local/bin/"
    echo ""
    echo -e "${YELLOW}Or install to your user bin (no sudo required):${NC}"
    echo -e "  mkdir -p ~/bin"
    echo -e "  cp $BINARY_PATH ~/bin/"
    echo -e "  # Add ~/bin to PATH in ~/.zshrc or ~/.bash_profile if needed"
fi

echo ""
echo -e "${GREEN}Build process complete!${NC}"
