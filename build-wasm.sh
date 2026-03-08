#!/usr/bin/env bash
set -euxo pipefail

cd "$(dirname "$0")/web"
npm run build:dev --workspace=logic-mesh
