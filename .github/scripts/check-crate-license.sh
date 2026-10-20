#!/usr/bin/env bash
# Checks that the .crate archive `cargo package` built for each CRATE
# carries the repository's LICENSE, byte for byte. BSD-3-Clause
# requires redistributions of the source to retain the license text,
# every copy downloaded from crates.io is one, and a published crate
# version can never be changed.
#
# block_macro/LICENSE is a symbolic link to the root LICENSE, which
# `cargo package` stores as a regular file. This also catches a
# checkout that turned the link into a file holding its target path
# (git on Windows without symlink support).
#
# Usage: .github/scripts/check-crate-license.sh CRATE...
#
# Run it after `cargo package -p CRATE...`, which writes the archives
# to target/package/. Reports each crate that fails on stderr and
# exits non-zero. Needs cargo, jq and tar.

set -euo pipefail

if (($# == 0)); then
	echo "usage: $0 CRATE..." >&2
	exit 2
fi

cd "$(dirname "$0")/../.."

# `--no-deps` reads only the workspace manifests: no network, no
# dependency resolution.
metadata=$(cargo metadata --no-deps --format-version 1)
target_dir=$(jq -er .target_directory <<<"$metadata")

failed=0

for crate in "$@"; do
	if ! version=$(jq -er --arg name "$crate" \
		'.packages[] | select(.name == $name) | .version' <<<"$metadata"); then
		echo "::error::there is no workspace crate named '$crate'" >&2
		failed=1
		continue
	fi

	archive="$target_dir/package/$crate-$version.crate"
	if [[ ! -f $archive ]]; then
		echo "::error::$archive is missing; run \`cargo package -p $crate\` first" >&2
		failed=1
		continue
	fi

	# A .crate is a gzipped tarball of the crate under `NAME-VERSION/`.
	license="$crate-$version/LICENSE"
	entries=$(tar -tzf "$archive")
	if ! grep -qxF "$license" <<<"$entries"; then
		echo "::error::the $crate $version crate has no LICENSE file" >&2
		failed=1
	elif ! tar -xzOf "$archive" "$license" | cmp -s - LICENSE; then
		echo "::error::the LICENSE in the $crate $version crate differs from the repository's LICENSE" >&2
		failed=1
	else
		echo "$crate $version: LICENSE ok"
	fi
done

if ((failed)); then
	exit 1
fi
