#!/usr/bin/env bash
# Bootstrap a local Stalwart v0.16 JMAP server for the carillon live tests,
# on port 8082 so it runs beside the other Pimalaya suites. Adapted from
# io-jmap's tests/stalwart.sh.
#
# Stalwart v0.16 dropped the REST `/api/principal` endpoint and the
# auto-provisioning script that v0.15 supported. This script does the
# equivalent over the new JMAP management surface (`urn:stalwart:jmap`),
# which is not yet documented but is what the new webadmin uses.
#
# Steps:
#   1. Write a minimal `config.json` (rocksdb data store only).
#   2. Start the container with `STALWART_RECOVERY_ADMIN=admin:test` so
#      a permanent admin exists from first boot (no bootstrap wizard).
#   3. Wait for `/.well-known/jmap` to respond, then resolve the
#      admin's JMAP account id.
#   4. Provision over JMAP `POST /jmap/`:
#        - x:Domain/set     create "pimalaya.org"
#        - x:Account/set    create test user with strong password
#
# Host port mapping:
#   8082 → admin HTTP (JMAP + webadmin at /admin)
#
# The chosen password is `P!malaya-test-2026`. Stalwart's password
# strength check rejects shorter / weaker secrets like `test`.

set -eu

NAME="carillon-tests"
ADMIN_PASS="test"
JMAP_PASS='P!malaya-test-2026'
ADMIN_PORT=8082
IMAGE="stalwartlabs/stalwart:v0.16-alpine"

CONFIG=$(mktemp)
trap 'rm -f "$CONFIG"' EXIT
printf '{"@type":"RocksDb","path":"/var/lib/stalwart/data"}\n' > "$CONFIG"
# mktemp defaults to mode 600; the stalwart UID inside the container
# needs read access on the bind-mounted config.
chmod 644 "$CONFIG"

docker rm -f "$NAME" >/dev/null 2>&1 || true
docker run -d --name "$NAME" --rm \
    -e "STALWART_RECOVERY_ADMIN=admin:${ADMIN_PASS}" \
    -v "${CONFIG}:/etc/stalwart/config.json:ro" \
    -p "${ADMIN_PORT}:8080" \
    "$IMAGE" >/dev/null

# Wait for the admin HTTP listener.
for _ in $(seq 1 30); do
    if curl -fsS -u "admin:${ADMIN_PASS}" \
        "http://localhost:${ADMIN_PORT}/.well-known/jmap" >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

# Resolve admin's JMAP account id from the session document.
acc=$(curl -fsSL -u "admin:${ADMIN_PASS}" \
    "http://localhost:${ADMIN_PORT}/.well-known/jmap" |
    jq -r '.accounts | keys[0]')

# Batch: create domain + user.
curl -fsS -u "admin:${ADMIN_PASS}" \
    -H 'Content-Type: application/json' \
    -d "{
      \"using\":[\"urn:ietf:params:jmap:core\",\"urn:stalwart:jmap\"],
      \"methodCalls\":[
        [\"x:Domain/set\",
          {\"accountId\":\"$acc\",\"create\":{
            \"d1\":{\"name\":\"pimalaya.org\"}
          }},\"0\"],
        [\"x:Account/set\",
          {\"accountId\":\"$acc\",\"create\":{
            \"u1\":{
              \"@type\":\"User\",
              \"name\":\"test\",
              \"domainId\":\"#d1\",
              \"credentials\":{
                \"0\":{\"@type\":\"Password\",\"secret\":\"${JMAP_PASS}\"}
              }
            }
          }},\"1\"]
      ]
    }" \
    "http://localhost:${ADMIN_PORT}/jmap/" |
    jq -e '.methodResponses[] | .[1] | (.created // {}) | length > 0' >/dev/null

echo "stalwart ready: jmap://test@pimalaya.org:${JMAP_PASS}@127.0.0.1:${ADMIN_PORT}"
