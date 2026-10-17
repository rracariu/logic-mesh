#!/usr/bin/env bash
# Checks that every version number a release ships agrees: the
# logic-mesh and logic_mesh_block_macro crate versions, the
# requirement logic-mesh puts on logic_mesh_block_macro (a leading `=`
# or `^` is ignored), and the npm package version. Given a release tag
# (vX.Y.Z or vX.Y.Z-pre), the tag must name that same version.
#
# The crates and the npm package are released together from one tag,
# so a mismatch anywhere fails both publish workflows instead of
# letting one registry get a release the other does not.
#
# Usage: .github/scripts/check-versions.sh [TAG]
#
# Prints the version on stdout when everything agrees; otherwise
# reports each mismatch on stderr and exits non-zero. Needs cargo and
# jq.

set -euo pipefail

cd "$(dirname "$0")/../.."

tag=${1:-}

# `--no-deps` reads only the workspace manifests: no network, no
# dependency resolution.
metadata=$(cargo metadata --no-deps --format-version 1)

# Prints the `.version` of the workspace package named $1.
crate_version() {
	jq -er --arg name "$1" '.packages[] | select(.name == $name) | .version' <<<"$metadata"
}

crate=$(crate_version logic-mesh)
macro=$(crate_version logic_mesh_block_macro)
# cargo metadata reports a bare `version = "2.0.0"` as `^2.0.0`.
macro_req=$(jq -er '.packages[] | select(.name == "logic-mesh")
	| .dependencies[] | select(.name == "logic_mesh_block_macro") | .req' <<<"$metadata")
macro_req_version=${macro_req#[=^]}
npm=$(jq -er .version web/packages/logic-mesh/package.json)

failed=0

# Reports a mismatch between what $2 says ($1) and the logic-mesh
# crate version.
expect_crate_version() {
	if [[ $1 != "$crate" ]]; then
		echo "::error::$2 is '$1', but the logic-mesh crate version is '$crate'" >&2
		failed=1
	fi
}

expect_crate_version "$macro" "the logic_mesh_block_macro crate version (block_macro/Cargo.toml)"
expect_crate_version "$macro_req_version" "the logic_mesh_block_macro requirement in Cargo.toml ('$macro_req')"
expect_crate_version "$npm" "the npm package version (web/packages/logic-mesh/package.json)"

if [[ -n $tag ]]; then
	if [[ $tag =~ ^v([0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?)$ ]]; then
		expect_crate_version "${BASH_REMATCH[1]}" "release tag '$tag'"
	else
		echo "::error::release tag '$tag' is not of the form vX.Y.Z or vX.Y.Z-pre" >&2
		failed=1
	fi
fi

if ((failed)); then
	exit 1
fi

echo "$crate"
