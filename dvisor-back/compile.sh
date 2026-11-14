#!/usr/bin/env bash
# Builds dvisor: the engine, in Rust, to WebAssembly (dvisor-web-lib/pkg),
# then the JavaScript library (dvisor-web-lib/dist). Runs on macOS, Linux
# and Windows (in Git Bash or WSL), from any folder.
#
# It first checks what the build needs and, for anything missing, says how
# to install it on this system. On the first run it also installs the
# library's npm dependencies.
set -e
cd "$(dirname "$0")"

case "$(uname -s)" in
    Darwin) os=macos ;;
    MINGW* | MSYS* | CYGWIN*) os=windows ;;
    *) os=linux ;;
esac

RUSTUP_UNIX="curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
RUSTUP_WINDOWS="run rustup-init.exe from https://rustup.rs, accepting the Visual Studio C++ Build Tools it offers"
NEW_TERMINAL="then open a new terminal"

missing=()

# need WHAT MACOS LINUX WINDOWS: notes that WHAT is missing, with the way to
# install it on this system.
need() {
    local how
    case $os in
        macos) how=$2 ;;
        linux) how=$3 ;;
        windows) how=$4 ;;
    esac
    missing+=("- $1:"$'\n'"    $how")
}

# A C toolchain, with which Rust links its build scripts. On Windows the
# Rust installer brings it.
if [ $os = macos ] && ! xcode-select -p > /dev/null 2>&1; then
    need "Xcode Command Line Tools" "xcode-select --install" "" ""
elif [ $os = linux ] && ! command -v cc > /dev/null; then
    need "a C compiler" "" \
        "sudo apt install build-essential (Debian, Ubuntu) or sudo dnf install gcc (Fedora)" ""
fi

# Rust, installed with rustup, which adds the WebAssembly target.
if ! command -v rustup > /dev/null || ! command -v cargo > /dev/null; then
    need "Rust, installed with rustup" \
        "$RUSTUP_UNIX, $NEW_TERMINAL" "$RUSTUP_UNIX, $NEW_TERMINAL" "$RUSTUP_WINDOWS, $NEW_TERMINAL"
elif ! rustup target list --installed | grep "^wasm32-unknown-unknown" > /dev/null; then
    need "Rust's WebAssembly target" "rustup target add wasm32-unknown-unknown" \
        "rustup target add wasm32-unknown-unknown" "rustup target add wasm32-unknown-unknown"
fi

if ! command -v wasm-pack > /dev/null; then
    need "wasm-pack" "cargo install wasm-pack" "cargo install wasm-pack" "cargo install wasm-pack"
fi

# Node.js 20 or later (what the Rollup terser plugin asks for), with npm.
node_major=$(node -p 'process.versions.node.split(".")[0]' 2> /dev/null | tr -dc 0-9 || true)
if [ -z "$node_major" ] || [ "$node_major" -lt 20 ] || ! command -v npm > /dev/null; then
    need "Node.js 20 or later, with npm${node_major:+ (found Node.js $node_major)}" \
        "brew install node, or the installer at https://nodejs.org" \
        "see https://nodejs.org/en/download (distribution packages may be older)" \
        "the installer at https://nodejs.org, $NEW_TERMINAL"
fi

if [ ${#missing[@]} -gt 0 ]; then
    echo "The build needs:"
    printf '%s\n' "${missing[@]}"
    echo "See Building in README.md."
    exit 1
fi

if [ ! -d ../dvisor-web-lib/node_modules ]; then
    echo "Installing the library's npm dependencies"
    (cd ../dvisor-web-lib && npm ci)
fi

echo "Building the engine (WebAssembly, release)"
rm -rf ../dvisor-web-lib/pkg ../dvisor-web-lib/dist
wasm-pack build --target web --out-dir ../dvisor-web-lib/pkg --release

echo "Building the library"
cd ../dvisor-web-lib
npm run build
