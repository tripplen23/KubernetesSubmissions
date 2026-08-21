#!/usr/bin/env sh
# Exercise 2.9 cron: create a "Read <URL>" todo every hour.
# Special:Random -> 302 redirect; read URL from Location header, then POST.
set -eu

BACKEND="${TODO_BACKEND_URL:?TODO_BACKEND_URL is not set}"

echo "[todo-cron] starting at $(date -Is)"
URL="$(curl -s -o /dev/null -w '%{redirect_url}' https://en.wikipedia.org/wiki/Special:Random)"
echo "[todo-cron] random article URL: ${URL}"

if [ -z "$URL" ]; then
  echo "[todo-cron] ERROR: no Location header returned by Special:Random" >&2
  exit 1
fi

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