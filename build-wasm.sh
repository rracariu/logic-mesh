#!/usr/bin/env bash
set -euxo pipefail

wasm-pack build --dev --out-dir ./web/module/pkg
mv ./web/module/pkg/*.{js,ts,wasm} ./web/module
cd ./web/module && npm version $(node -p "require('./pkg/package.json').version") --allow-same-version && rm -rf ./pkg