#!/usr/bin/env sh
# Exercise 2.9 — CronJob: create a "Read <URL>" todo every hour.
#
# Fetches a random Wikipedia article via https://en.wikipedia.org/wiki/
# Special:Random (which responds with a 302 redirect), reads the URL from
# the Location header, then POSTs {"title": "Read <url>"} to the
# todo-backend service.
set -eu

# TODO_BACKEND_URL must be injected by the CronJob (e.g.
# http://todo-backend-svc:2345). Fail loudly if it is missing.
BACKEND="${TODO_BACKEND_URL:?TODO_BACKEND_URL is not set}"

echo "[todo-cron] starting at $(date -Is)"

# `curl -w '%{redirect_url}'` prints the value of the Location header of
# the 302 response — i.e. the random article's URL (we do NOT follow it,
# we just read where it points).
URL="$(curl -s -o /dev/null -w '%{redirect_url}' https://en.wikipedia.org/wiki/Special:Random)"
echo "[todo-cron] random article URL: ${URL}"

if [ -z "$URL" ]; then
  echo "[todo-cron] ERROR: no Location header returned by Special:Random" >&2
  exit 1
fi

# Create the todo through the backend (stored in Postgres).
# JSON-encode the URL into the title.
TITLE="Read $URL"
HTTP_CODE="$(curl -s -o /dev/null -w '%{http_code}' \
  -X POST "${BACKEND}/todos" \
  -H 'Content-Type: application/json' \
  -d "{\"title\": \"${TITLE}\"}")"

echo "[todo-cron] POST ${BACKEND}/todos -> ${HTTP_CODE}"
if [ "$HTTP_CODE" != "201" ] && [ "$HTTP_CODE" != "200" ]; then
  echo "[todo-cron] ERROR: backend returned ${HTTP_CODE}" >&2
  exit 1
fi

echo "[todo-cron] created todo: ${TITLE}"
