#!/usr/bin/env bash
# Checks each CRATE's keywords and categories against the limits
# crates.io enforces when it receives an upload. `cargo package` and
# `cargo publish --dry-run` never contact crates.io, so they accept a
# manifest that the real upload then rejects, after other crates in
# the release may already be published.
#
# crates.io's rules:
# - at most 5 keywords, each at most 20 characters, made of ASCII
#   letters, digits, `_`, `-` and `+`, and starting with a letter or
#   digit;
# - at most 5 categories.
#
# Usage: .github/scripts/check-crate-metadata.sh CRATE...
#
# Reports each problem on stderr and exits non-zero. Needs cargo and
# jq.

set -euo pipefail

if (($# == 0)); then
	echo "usage: $0 CRATE..." >&2
	exit 2
fi

cd "$(dirname "$0")/../.."

# `--no-deps` reads only the workspace manifests: no network, no
# dependency resolution.
metadata=$(cargo metadata --no-deps --format-version 1)

max_keywords=5
max_keyword_len=20
max_categories=5

failed=0

for crate in "$@"; do
	if ! package=$(jq -ec --arg name "$crate" \
		'.packages[] | select(.name == $name)' <<<"$metadata"); then
		echo "::error::there is no workspace crate named '$crate'" >&2
		failed=1
		continue
	fi

	crate_failed=0

	keywords=()
	while IFS= read -r keyword; do
		keywords+=("$keyword")
	done < <(jq -r '.keywords[]' <<<"$package")

	if ((${#keywords[@]} > max_keywords)); then
		echo "::error::$crate has ${#keywords[@]} keywords; crates.io allows at most $max_keywords" >&2
		crate_failed=1
	fi

	for keyword in ${keywords[@]+"${keywords[@]}"}; do
		if ((${#keyword} > max_keyword_len)); then
			echo "::error::$crate keyword '$keyword' is ${#keyword} characters long; crates.io allows at most $max_keyword_len" >&2
			crate_failed=1
		fi
		if [[ ! $keyword =~ ^[A-Za-z0-9][A-Za-z0-9_+-]*$ ]]; then
			echo "::error::$crate keyword '$keyword' must start with an ASCII letter or digit and use only letters, digits, '_', '-' and '+'" >&2
			crate_failed=1
		fi
	done

	categories=$(jq -r '.categories | length' <<<"$package")
	if ((categories > max_categories)); then
		echo "::error::$crate has $categories categories; crates.io allows at most $max_categories" >&2
		crate_failed=1
	fi

	if ((crate_failed)); then
		failed=1
	else
		echo "$crate: keywords and categories ok"
	fi
done

if ((failed)); then
	exit 1
fi
