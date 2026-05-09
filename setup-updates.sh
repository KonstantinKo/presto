#!/bin/bash

# Configuration script for Presto Update System
# This script will guide you through the configuration of automatic updates

echo "🍅 Presto - Update System Setup"
echo "=================================================="
echo ""

# Function to request input
read_input() {
    local prompt="$1"
    local variable_name="$2"
    local default_value="$3"
    
    if [ -n "$default_value" ]; then
        read -p "$prompt [$default_value]: " input
        if [ -z "$input" ]; then
            input="$default_value"
        fi
    else
        read -p "$prompt: " input
    fi
    
    printf -v "$variable_name" '%s' "$input"
}

# Gather information
echo "1. GitHub Repository Configuration"
echo "-----------------------------------"
read_input "GitHub Username" github_username
read_input "Repository name" github_repo "presto"

echo ""
echo "2. Key Configuration"
echo "------------------------"
read_input "Key file name" key_name "presto_signing_key"

# Create the keys directory
key_dir="$HOME/.tauri"
mkdir -p "$key_dir"

echo ""
echo "📝 Generating signing keys..."

# Check if tauri CLI is available
if ! command -v tauri &> /dev/null; then
    echo "❌ Tauri CLI not found. Installing..."
    npm install --save-dev @tauri-apps/cli@latest

    if ! command -v npx &> /dev/null; then
        echo "❌ NPM not found. Please install Node.js first."
        exit 1
    fi

    # Use npx if tauri is not in PATH
    TAURI_CMD="npx tauri"
else
    TAURI_CMD="tauri"
fi

# Generate keys
echo "🔑 Generating keypair..."
if $TAURI_CMD signer generate -w "$key_dir/$key_name"; then
    echo "✅ Keys generated successfully!"

    # Get the public key
    echo ""
    echo "🔑 Your public key is:"
    echo "----------------------------------------"
    public_key=$($TAURI_CMD signer sign -k "$key_dir/$key_name" --password "" 2>/dev/null | head -1)
    echo "$public_key"
    echo "----------------------------------------"
    
    # Update tauri.conf.json
    echo ""
    echo "📝 Updating configuration..."

    # Replace placeholders in the configuration file
    config_file="src-tauri/tauri.conf.json"
    if [ -f "$config_file" ]; then
        # Backup of the original file
        cp "$config_file" "$config_file.backup"

        # Escape values for safe use in sed substitutions (/, &, \)
        sed_username=$(printf '%s' "$github_username" | sed 's/[\/&]/\\&/g')
        sed_repo=$(printf '%s' "$github_repo" | sed 's/[\/&]/\\&/g')
        sed_pubkey=$(printf '%s' "$public_key" | sed 's/[\/&]/\\&/g')

        # Substitutions
        sed -i.tmp "s/{{OWNER}}/$sed_username/g" "$config_file"
        sed -i.tmp "s/{{REPO}}/$sed_repo/g" "$config_file"
        sed -i.tmp "s/YOUR_PUBLIC_KEY_HERE/$sed_pubkey/g" "$config_file"
        rm "$config_file.tmp" 2>/dev/null

        echo "✅ Configuration updated!"
    else
        echo "⚠️  File $config_file not found"
    fi

    # Update main.js with repository links
    main_js_file="src/main.js"
    if [ -f "$main_js_file" ]; then
        sed -i.tmp "s/YOUR_USERNAME/$sed_username/g" "$main_js_file"
        sed -i.tmp "s/YOUR_REPO/$sed_repo/g" "$main_js_file"
        rm "$main_js_file.tmp" 2>/dev/null
        echo "✅ Repository links updated!"
    fi

    # Update the update manager
    update_manager_file="src/managers/update-manager-global.js"
    if [ -f "$update_manager_file" ]; then
        sed -i.tmp "s/USERNAME\/REPOSITORY/$sed_username\/$sed_repo/g" "$update_manager_file"
        rm "$update_manager_file.tmp" 2>/dev/null
        echo "✅ Update manager configured!"
    fi

    echo ""
    echo "🎉 Configuration complete!"
    echo ""
    echo "📋 Next steps:"
    echo "1. Add these secrets to your GitHub repository:"
    echo "   - TAURI_SIGNING_PRIVATE_KEY: (content of $key_dir/$key_name)"
    echo "   - TAURI_SIGNING_PRIVATE_KEY_PASSWORD: (leave empty if you did not set a password)"
    echo ""
    echo "2. To get the private key:"
    echo "   cat $key_dir/$key_name"
    echo ""
    echo "3. Create a release on GitHub to test updates:"
    echo "   git tag v0.2.0"
    echo "   git push origin v0.2.0"
    echo ""
    echo "4. The app will automatically check for updates at startup"
    echo ""
    echo "⚠️  IMPORTANT: Never commit the private key to the repository!"
    echo "   The key is saved in: $key_dir/$key_name"

else
    echo "❌ Error generating keys"
    exit 1
fi
