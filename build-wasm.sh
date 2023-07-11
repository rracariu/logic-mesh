#!/usr/bin/env bash
wasm-pack build --dev --out-dir ./web/module/pkg
mv ./web/module/pkg/*.{js,ts,wasm} ./web/module && rm -rf ./web/module/pkg