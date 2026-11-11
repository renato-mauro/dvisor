#!/usr/bin/env bash

echo "Building Dvisor WebAssembly package (release)"
rm -r ../dvisor-web-lib/pkg
rm -r ../dvisor-web-lib/dist
wasm-pack build --target web --out-dir ../dvisor-web-lib/pkg --release
cd ../dvisor-web-lib
npm run build
cd ../dvisor-back

