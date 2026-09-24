#!/bin/sh
# The sidecar: wait a random while, then replace the served page with a random
# Wikipedia article, forever.
#
# The exercise's wait is 5 to 15 minutes. MIN_WAIT_SECONDS/MAX_WAIT_SECONDS are
# here so the same loop can be watched in seconds instead of waiting a quarter
# of an hour for the first fetch.
set -eu

MIN="${MIN_WAIT_SECONDS:-300}"
MAX="${MAX_WAIT_SECONDS:-900}"
TARGET="${WWW_DIR:-/www}/index.html"
URL="${TARGET_URL:-https://en.wikipedia.org/wiki/Special:Random}"
UA="${USER_AGENT:-dwk-5.4-lab/1.0}"

if [ "$MAX" -lt "$MIN" ]; then
  echo "MAX_WAIT_SECONDS ($MAX) is below MIN_WAIT_SECONDS ($MIN)" >&2
  exit 1
fi

while true; do
  wait=$(( MIN + RANDOM % (MAX - MIN + 1) ))
  echo "waiting ${wait}s before the next fetch"
  sleep "$wait"

  # Write beside the target and move it into place, so nginx never serves a
  # half-written article. Special:Random redirects, hence -L.
  tmp="${TARGET}.tmp"
  if curl -sSL -A "$UA" -o "$tmp" "$URL" \
      -w "fetched %{url_effective} (http %{http_code}, %{size_download} bytes)\n"; then
    mv "$tmp" "$TARGET"
    echo "now serving it as $TARGET"
  else
    echo "fetch failed, the previous page stays" >&2
    rm -f "$tmp"
  fi
done
