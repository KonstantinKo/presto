#!/bin/bash

# Verification script for Tempo update system configuration
# This script checks if everything is properly set up for automatic updates

echo "🔍 Tempo Update System Verification"
echo "=================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    local status=$1
    local message=$2
    if [ "$status" = "ok" ]; then
        echo -e "${GREEN}✅${NC} $message"
    elif [ "$status" = "warning" ]; then
        echo -e "${YELLOW}⚠️${NC}  $message"
    else
        echo -e "${RED}❌${NC} $message"
    fi
}

# Function to check if a file exists
check_file() {
    local file=$1
    local description=$2
    if [ -f "$file" ]; then
        print_status "ok" "$description found"
        return 0
    else
        print_status "error" "$description not found at $file"
        return 1
    fi
}

# Function to check if a command exists
check_command() {
    local cmd=$1
    local description=$2
    if command -v "$cmd" &> /dev/null; then
        print_status "ok" "$description available"
        return 0
    else
        print_status "error" "$description not found"
        return 1
    fi
}

echo "📋 Checking Dependencies"
echo "------------------------"

# Check for required commands
check_command "npm" "npm"
check_command "npx" "npx"
check_command "git" "git"

# Check if tauri CLI is available
if npx tauri --version &> /dev/null; then
    print_status "ok" "Tauri CLI available"
else
    print_status "error" "Tauri CLI not available"
fi

echo ""
echo "📁 Checking Project Files"
echo "-------------------------"

# Check critical files
check_file "src-tauri/tauri.conf.json" "Tauri configuration"
check_file ".github/workflows/release.yml" "GitHub Actions workflow"
check_file "src-tauri/capabilities/default.json" "Tauri capabilities"
check_file "src/managers/update-manager-global.js" "Update manager"
check_file "src/components/update-notification.js" "Update notification component"

echo ""
echo "🔧 Checking Configuration"
echo "-------------------------"

# Check tauri.conf.json for updater configuration
if [ -f "src-tauri/tauri.conf.json" ]; then
    if grep -q '"updater"' src-tauri/tauri.conf.json; then
        print_status "ok" "Updater plugin configured in tauri.conf.json"
        
        # Check if pubkey is set
        if grep -q '"pubkey": ""' src-tauri/tauri.conf.json; then
            print_status "warning" "Public key is empty in tauri.conf.json"
        elif grep -q '"pubkey":' src-tauri/tauri.conf.json; then
            print_status "ok" "Public key is set in tauri.conf.json"
        fi
        
        # Check endpoint
        if grep -q 'YOUR_USERNAME' src-tauri/tauri.conf.json; then
            print_status "warning" "GitHub repository placeholders not replaced"
        elif grep -q 'github.com' src-tauri/tauri.conf.json; then
            print_status "ok" "GitHub endpoint configured"
        fi
    else
        print_status "error" "Updater plugin not configured in tauri.conf.json"
    fi
fi

# Check capabilities
if [ -f "src-tauri/capabilities/default.json" ]; then
    if grep -q 'updater:allow-check' src-tauri/capabilities/default.json; then
        print_status "ok" "Updater permissions configured"
    else
        print_status "error" "Updater permissions missing in capabilities"
    fi
fi

echo ""
echo "🔑 Checking Signing Keys"
echo "------------------------"

# Check for signing keys
KEY_PATH="$HOME/.tauri/tempo_signing_key"
PUB_KEY_PATH="$HOME/.tauri/tempo_signing_key.pub"

if [ -f "$KEY_PATH" ]; then
    print_status "ok" "Private signing key found"
else
    print_status "error" "Private signing key not found at $KEY_PATH"
fi

if [ -f "$PUB_KEY_PATH" ]; then
    print_status "ok" "Public signing key found"
    echo -e "${BLUE}📄 Public key content:${NC}"
    cat "$PUB_KEY_PATH"
    echo ""
else
    print_status "error" "Public signing key not found at $PUB_KEY_PATH"
fi

echo ""
echo "📦 Checking Dependencies"
echo "------------------------"

# Check package.json for required dependencies
if [ -f "package.json" ]; then
    if grep -q '@tauri-apps/plugin-updater' package.json; then
        print_status "ok" "Updater plugin dependency found"
    else
        print_status "warning" "Updater plugin dependency not found in package.json"
    fi
    
    if grep -q '@tauri-apps/plugin-dialog' package.json; then
        print_status "ok" "Dialog plugin dependency found"
    else
        print_status "warning" "Dialog plugin dependency not found in package.json"
    fi
fi

echo ""
echo "🚀 Next Steps"
echo "-------------"

if [ -f "$KEY_PATH" ] && [ -f "$PUB_KEY_PATH" ]; then
    echo -e "${GREEN}1.${NC} Add these secrets to your GitHub repository:"
    echo "   - TAURI_SIGNING_PRIVATE_KEY: (content of $KEY_PATH)"
    echo "   - TAURI_SIGNING_PRIVATE_KEY_PASSWORD: (your key password)"
    echo ""
fi

echo -e "${GREEN}2.${NC} Test the setup by creating a release:"
echo "   git tag v0.1.1"
echo "   git push origin v0.1.1"
echo ""

echo -e "${GREEN}3.${NC} Monitor the GitHub Actions workflow"
echo ""

echo -e "${GREEN}4.${NC} Test update checking in the compiled app"
echo ""

echo "🎉 Verification complete!"
echo "For detailed setup instructions, see: UPDATES.md"
