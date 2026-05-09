#!/bin/bash

# Script to generate signing keys for Tauri updates
# This script must be run in the project directory

echo "🔐 Generating update signing keys for Tauri..."

# Check if npm is available
if ! command -v npm &> /dev/null; then
    echo "❌ npm not found. Please install Node.js first."
    exit 1
fi

# Check if --force option was passed
FORCE_OPTION=""
for arg in "$@"; do
    if [ "$arg" = "--force" ]; then
        FORCE_OPTION="--force"
        echo "⚠️ Force option detected. Will overwrite existing keys."
        break
    fi
done

# Ensure npx is available
if ! command -v npx &> /dev/null; then
    echo "❌ npx not found. Please install Node.js first."
    exit 1
fi

# Generate signing keys
echo "📝 Generating signing keypair..."
if npx tauri signer generate -w ~/.tauri/presto_signing_key ${FORCE_OPTION:+"$FORCE_OPTION"}; then
    echo "✅ Keys generated successfully!"
    echo ""
    echo "🔑 Your public key is:"
    npx tauri signer sign -k ~/.tauri/presto_signing_key --password "" | head -1
    echo ""
    echo "📋 Next steps:"
    echo "1. Copy the public key above"
    echo "2. Replace 'YOUR_PUBLIC_KEY_HERE' in src-tauri/tauri.conf.json with your public key"
    echo "3. Keep your private key secure (~/.tauri/presto_signing_key)"
    echo "4. Add the private key to your GitHub Actions secrets as TAURI_SIGNING_PRIVATE_KEY"
    echo ""
    echo "⚠️  Important: Never commit your private key to version control!"
else
    echo "❌ Failed to generate keys"
    exit 1
fi
