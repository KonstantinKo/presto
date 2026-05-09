# Automatic Updates Configuration

This guide will help you configure the automatic update system for your Presto application.

## 🔐 Step 1: Generate Signing Keys

Before publishing releases with automatic updates, you need to generate a key pair to sign the updates:

```bash
# Run the key generation script
./generate-keys.sh
```

This script will generate:
- A **private key** (keep it secret!)
- A **public key** (to be inserted in the configuration)

## 🔧 Step 2: Configure tauri.conf.json

> **Note for contributors to the main repository**: `src-tauri/tauri.conf.json` already contains a real public key and points to the `murdercode/presto` release endpoint. No changes are needed unless you are setting up your own fork.

If you are configuring a fork:

1. Copy the public key generated in the previous step
2. Open `src-tauri/tauri.conf.json`
3. Update the `plugins.updater` section with your own values:

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/YOUR_USERNAME/YOUR_REPOSITORY/releases/latest/download/latest-{target}.json"
      ],
      "pubkey": "YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

Replace:
- `YOUR_USERNAME` with your GitHub username
- `YOUR_REPOSITORY` with your repository name  
- `YOUR_PUBLIC_KEY_HERE` with the generated public key

## 🚀 Step 3: Configure GitHub Actions

### Add Secrets

In your GitHub repository, go to **Settings** → **Secrets and variables** → **Actions** and add:

1. **TAURI_SIGNING_PRIVATE_KEY**: The generated private key (content of `~/.tauri/presto_signing_key` file)
2. **TAURI_SIGNING_PRIVATE_KEY_PASSWORD**: The key password (leave empty if you didn't set one)

### Verify the Workflow

The `.github/workflows/release.yml` file is already configured to:
- Build the app for macOS (both Apple Silicon and Intel)
- Sign files with your private key
- Create a release on GitHub
- Automatically publish assets

> **Note**: Windows and Linux builds are not currently included in the release workflow.

## 📦 Step 4: Create a Release

To trigger the build and release process:

```bash
# Create and push a version tag
git tag v0.2.0
git push origin v0.2.0
```

Or create a release directly from GitHub.

## 🔄 How Updates Work

### For Users

1. The app automatically checks for updates on startup
2. If an update is found, a notification appears
3. Users can choose to download and install immediately
4. Download happens in background with a progress bar
5. Update is applied on app restart

### For Developers

1. When you publish a release on GitHub, files are automatically signed
2. The app checks the GitHub API endpoint for new releases
3. Verifies the signature for security
4. Downloads and installs the update automatically

## ⚙️ Advanced Configurations

### Automatic Checking

Users can configure automatic checking in settings:
- **Check for updates automatically**: Checks every hour
- **Include pre-release versions**: Also beta/RC versions

### Custom Endpoints

You can modify the update endpoint in `tauri.conf.json` for:
- Private repositories
- Custom distribution servers
- Different release channels

### Example for Private Repository

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://api.github.com/repos/username/private-repo/releases/latest"
      ],
      "pubkey": "your_public_key",
      "headers": {
        "Authorization": "token ghp_xxxxxxxxxxxx"
      }
    }
  }
}
```

## 🛡️ Security

- **Never commit the private key** to the repository
- Always use GitHub Secrets to store keys
- Signatures ensure updates come from you
- The app will reject unsigned or invalid signature updates

## 🐛 Troubleshooting

### App doesn't find updates

1. Verify that the repository URL in `tauri.conf.json` is correct
2. Make sure there's at least one published release on GitHub
3. Check console logs for errors

### Signature errors

1. Verify that the public key in `tauri.conf.json` matches the private one
2. Make sure GitHub Actions has access to the private key
3. Check that the key hasn't expired

### Download fails

1. Check internet connection
2. Verify that release files have been uploaded correctly
3. Check repository permissions (public vs private)

## 📚 References

- [Tauri Updater Documentation](https://tauri.app/v1/guides/distribution/updater)
- [GitHub Actions for Tauri](https://tauri.app/v1/guides/building/cross-platform)
- [GitHub Release Management](https://docs.github.com/en/repositories/releasing-projects-on-github)

## 🔄 Updating this Configuration

If you modify the update system:

1. Update this documentation file
2. Test the process with a trial release
3. Inform users of changes in release notes
