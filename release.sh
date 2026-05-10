#!/bin/bash

# Automation script for releasing Presto
# Handles versioning, commit, tag, push, and build automatically

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
print_step() {
    echo -e "${BLUE}🔄 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to increment version
increment_version() {
    local version=$1
    local type=$2

    IFS='.' read -ra VERSION_PARTS <<< "$version"
    local major=${VERSION_PARTS[0]}
    local minor=${VERSION_PARTS[1]}
    local patch=${VERSION_PARTS[2]}

    case $type in
        "major")
            major=$((major + 1))
            minor=0
            patch=0
            ;;
        "minor")
            minor=$((minor + 1))
            patch=0
            ;;
        "patch")
            patch=$((patch + 1))
            ;;
        *)
            echo "Invalid version type: $type"
            exit 1
            ;;
    esac

    echo "$major.$minor.$patch"
}

# Function to update version in files
update_version_in_files() {
    local old_version=$1
    local new_version=$2

    print_step "Updating version from $old_version to $new_version..."

    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        sed -i '' "s/version = \"$old_version\"/version = \"$new_version\"/" src-tauri/Cargo.toml
        sed -i '' "s/\"version\": \"$old_version\"/\"version\": \"$new_version\"/" src-tauri/tauri.conf.json
        sed -i '' "s/version = \"$old_version\"/version = \"$new_version\"/" src/Cargo.toml
    else
        # Linux
        sed -i "s/version = \"$old_version\"/version = \"$new_version\"/" src-tauri/Cargo.toml
        sed -i "s/\"version\": \"$old_version\"/\"version\": \"$new_version\"/" src-tauri/tauri.conf.json
        sed -i "s/version = \"$old_version\"/version = \"$new_version\"/" src/Cargo.toml
    fi

    print_success "Version updated in configuration files"
}

# Function to get current version
get_current_version() {
    grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/'
}

# Function to check if the directory is clean
check_git_status() {
    if [[ -n $(git status --porcelain) ]]; then
        print_warning "There are uncommitted changes. Do you want to continue? (y/N)"
        read -r response
        if [[ ! "$response" =~ ^[Yy]$ ]]; then
            print_error "Operation cancelled"
            exit 1
        fi
    fi
}

# Function to commit and tag
commit_and_tag() {
    local version=$1
    local message="$2"

    print_step "Adding modified files to git..."
    git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src/Cargo.toml

    # If there are other modified files, ask whether to add them
    if [[ -n $(git status --porcelain | grep -v "Cargo.toml\|Cargo.lock\|tauri.conf.json") ]]; then
        print_warning "There are other modified files. Do you want to add them to the commit? (y/N)"
        read -r response
        if [[ "$response" =~ ^[Yy]$ ]]; then
            git add .
        fi
    fi

    print_step "Committing changes..."
    git commit -m "chore: release v$version${message:+ - $message}"

    print_step "Creating tag v$version..."
    git tag -a "v$version" -m "Release v$version${message:+ - $message}"

    print_success "Commit and tag created"
}

# Function to push changes
push_changes() {
    local version=$1

    print_step "Pushing main commit..."
    git push origin main

    print_step "Pushing tag v$version..."
    git push origin "v$version"

    print_success "Push complete"
}

# Function to update the Homebrew tap
update_homebrew_tap() {
    local version=$1
    local tap_repo_path="${PRESTO_HOMEBREW_TAP:-../homebrew-presto}"

    print_step "Updating Homebrew tap..."

    if [ ! -d "$tap_repo_path" ]; then
        print_warning "Homebrew tap repository not found: $tap_repo_path"
        print_warning "Skipping Homebrew tap update"
        return 0
    fi

    # Go to the tap directory
    cd "$tap_repo_path"

    # Run the update script
    if [ -x "./update-homebrew-tap.sh" ]; then
        ./update-homebrew-tap.sh "$version"
        print_success "Homebrew tap updated to version $version"
    else
        print_warning "Tap update script not found or not executable"
    fi

    # Return to the original directory
    cd - > /dev/null
}

# Function to build the app
build_app() {
    print_step "Starting application build..."
    npm run tauri build -- --bundles app
    print_success "Build complete"
}

# Function to open GitHub releases
open_github_releases() {
    local repo_url=$(git config --get remote.origin.url)
    if [[ $repo_url == *"github.com"* ]]; then
        # Convert SSH URL to HTTPS
        repo_url=$(echo $repo_url | sed 's/git@github.com:/https:\/\/github.com\//' | sed 's/\.git$//')
        local releases_url="$repo_url/releases/new"
        print_step "Opening GitHub releases page..."
        if command -v open &> /dev/null; then
            open "$releases_url"
        elif command -v xdg-open &> /dev/null; then
            xdg-open "$releases_url"
        else
            echo "Open manually: $releases_url"
        fi
    fi
}

# Main function
main() {
    echo -e "${BLUE}"
    echo "🚀 Automated Release Script for Presto"
    echo "=======================================${NC}"

    # Check if we are in a git repo
    if [[ ! -d .git ]]; then
        print_error "Not in a git repository"
        exit 1
    fi

    # Get current version
    current_version=$(get_current_version)
    print_step "Current version: $current_version"

    # Ask release type
    echo ""
    echo "What type of release do you want to make?"
    echo "1) Patch (${current_version} → $(increment_version $current_version patch))"
    echo "2) Minor (${current_version} → $(increment_version $current_version minor))"
    echo "3) Major (${current_version} → $(increment_version $current_version major))"
    echo "4) Specific version"
    echo "5) Build only (without updating version)"
    echo ""
    read -p "Select an option (1-5): " choice

    case $choice in
        1)
            release_type="patch"
            new_version=$(increment_version $current_version patch)
            ;;
        2)
            release_type="minor"
            new_version=$(increment_version $current_version minor)
            ;;
        3)
            release_type="major"
            new_version=$(increment_version $current_version major)
            ;;
        4)
            read -p "Enter the new version (format x.y.z): " new_version
            if [[ ! $new_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                print_error "Invalid version format"
                exit 1
            fi
            release_type="custom"
            ;;
        5)
            print_step "Build only without version update..."
            build_app
            print_success "Build complete!"
            exit 0
            ;;
        *)
            print_error "Invalid option"
            exit 1
            ;;
    esac

    # Optional release message
    read -p "Optional message for this release: " release_message

    echo ""
    print_step "Planned release: $current_version → $new_version"
    if [[ -n "$release_message" ]]; then
        echo "Message: $release_message"
    fi
    echo ""

    # Final confirmation
    read -p "Continue with the release? (y/N): " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        print_error "Release cancelled"
        exit 1
    fi

    # Check git status
    check_git_status

    # Update version in files
    update_version_in_files $current_version $new_version

    # Build
    build_app

    # Commit and tag
    commit_and_tag $new_version "$release_message"

    # Push
    push_changes $new_version

    # Update Homebrew tap
    update_homebrew_tap $new_version

    # Open GitHub releases
    print_step "Do you want to open the GitHub releases page to complete the release? (Y/n)"
    read -r open_github
    if [[ ! "$open_github" =~ ^[Nn]$ ]]; then
        open_github_releases
    fi

    echo ""
    print_success "🎉 Release v$new_version completed successfully!"
    echo ""
    echo "Next steps:"
    echo "1. Verify that the build completed correctly"
    echo "2. If you opened GitHub, create the release with the compiled files"
    echo "3. Tag v$new_version has been created for the automatic update system"
    echo "4. Test the automatic app update"
    echo ""

    # Show information about generated files
    build_artifacts=$(find src-tauri/target -name "*.app.tar.gz" -o -name "*.app" -o -name "*.deb" -o -name "*.AppImage" 2>/dev/null | head -5)
    if [ -n "$build_artifacts" ]; then
        echo "Generated build files:"
        echo "$build_artifacts"
    fi
}

# Command-line parameter handling
if [[ $# -gt 0 ]]; then
    case $1 in
        "--help"|"-h")
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --patch     Increment patch version"
            echo "  --minor     Increment minor version"
            echo "  --major     Increment major version"
            echo "  --version X.Y.Z  Set specific version"
            echo "  --build-only     Build only without updating version"
            echo "  --help      Show this message"
            echo ""
            echo "Examples:"
            echo "  $0              # Interactive mode"
            echo "  $0 --patch      # Automatic patch release"
            echo "  $0 --version 1.0.0  # Specific version"
            exit 0
            ;;
        "--patch")
            current_version=$(get_current_version)
            new_version=$(increment_version $current_version patch)
            update_version_in_files $current_version $new_version
            commit_and_tag $new_version
            push_changes $new_version
            build_app
            update_homebrew_tap $new_version
            print_success "Patch release v$new_version complete!"
            ;;
        "--minor")
            current_version=$(get_current_version)
            new_version=$(increment_version $current_version minor)
            update_version_in_files $current_version $new_version
            commit_and_tag $new_version
            push_changes $new_version
            build_app
            update_homebrew_tap $new_version
            print_success "Minor release v$new_version complete!"
            ;;
        "--major")
            current_version=$(get_current_version)
            new_version=$(increment_version $current_version major)
            update_version_in_files $current_version $new_version
            commit_and_tag $new_version
            push_changes $new_version
            build_app
            update_homebrew_tap $new_version
            print_success "Major release v$new_version complete!"
            ;;
        "--version")
            if [[ -z $2 ]] || [[ ! $2 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                print_error "Version not specified or invalid format"
                exit 1
            fi
            current_version=$(get_current_version)
            new_version=$2
            update_version_in_files $current_version $new_version
            commit_and_tag $new_version
            push_changes $new_version
            build_app
            update_homebrew_tap $new_version
            print_success "Release v$new_version complete!"
            ;;
        "--build-only")
            build_app
            print_success "Build complete!"
            ;;
        *)
            print_error "Unrecognized option: $1"
            echo "Use --help to see available options"
            exit 1
            ;;
    esac
else
    # Interactive mode
    main
fi
