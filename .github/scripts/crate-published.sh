#!/usr/bin/env bash
# Reports whether version VERSION of crate CRATE is already on
# crates.io by printing `true` or `false`. The publish workflow uses it
# to skip a crate that an earlier run of the same release already
# published, so re-running a release is safe.
#
# Any answer other than "found" or "not found" (rate limiting, an
# outage) is an error rather than a guess.
#
# Usage: .github/scripts/crate-published.sh CRATE VERSION

set -euo pipefail

if (($# != 2)); then
	echo "usage: $0 CRATE VERSION" >&2
	exit 2
fi

crate=$1
version=$2

# The crates.io API rejects requests without a User-Agent that
# identifies the caller. `--retry` covers timeouts, 429 and 5xx.
status=$(curl --silent --show-error --retry 3 \
	--user-agent "logic-mesh release workflow (https://github.com/rracariu/logic-mesh)" \
	--output /dev/null --write-out '%{http_code}' \
	"https://crates.io/api/v1/crates/$crate/$version")

case $status in
200) echo true ;;
404) echo false ;;
*)
	echo "::error::crates.io answered HTTP $status for $crate $version" >&2
	exit 1
	;;
esac
