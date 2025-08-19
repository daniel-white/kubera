#!/bin/sh
# vale-gateway readiness probe script
#
# This script checks the readiness of multiple listeners by making HTTP(S) requests
# to their configured endpoints and verifying the expected status code.
#
# Usage:
#   Set the VALE_GATEWAY_LISTENERS environment variable to a semicolon-delimited list of listeners.
#   Each listener entry is a comma-separated string:
#     listener_name,protocol,port,path,expected_status_code
#
#   Example:
#     export VALE_GATEWAY_LISTENERS="admin,http,8080,/healthz,200;public,https,8443,/ready,204"
#
#   Run the script:
#     ./readiness_probe.sh
#
# Exit codes:
#   0 - All listeners responded with expected status codes
#   1 - Any listener failed (timeout, protocol mismatch, wrong status code)
#
# Requirements:
#   - curl (should be present in most container images)
#
# Output:
#   Prints status for each listener. On failure, prints error and exits 1.

set -eu

if [ -z "${VALE_GATEWAY_LISTENERS:-}" ]; then
  echo "Error: VALE_GATEWAY_LISTENERS environment variable is not set."
  exit 1
fi

# Configurable timeout per request (seconds)
CURL_TIMEOUT=3

failures=0

echo "Starting readiness probe for listeners: $VALE_GATEWAY_LISTENERS"

# POSIX-compliant loop over entries
IFS=';'
for entry in $VALE_GATEWAY_LISTENERS; do
  # Skip empty entries
  [ -z "$entry" ] && continue
  old_ifs="$IFS"
  IFS=','
  set -- $entry
  IFS="$old_ifs"
  name="$1"
  proto="$2"
  port="$3"
  path="$4"
  expect_code="$5"
  if [ -z "$name" ] || [ -z "$proto" ] || [ -z "$port" ] || [ -z "$path" ] || [ -z "$expect_code" ]; then
    echo "Error: Invalid entry format: $entry"
    failures=1
    continue
  fi
  if [ "$proto" != "http" ] && [ "$proto" != "https" ]; then
    echo "Error: Protocol must be 'http' or 'https' for listener '$name'"
    failures=1
    continue
  fi
  url="$proto://127.0.0.1:$port$path"
  echo "Probing $name: $url (expect $expect_code)"
  status=`curl -k -s -o /dev/null -w "%{http_code}" --max-time $CURL_TIMEOUT "$url" 2>/dev/null || echo "curl_error"`
  if [ "$status" = "curl_error" ]; then
    echo "Error: Listener '$name' did not respond within $CURL_TIMEOUT seconds."
    failures=1
    continue
  fi
  if [ "$status" != "$expect_code" ]; then
    echo "Error: Listener '$name' responded with status $status (expected $expect_code)."
    failures=1
    continue
  fi
  echo "Listener '$name' is ready."
done

if [ "$failures" -eq 0 ]; then
  echo "All listeners are ready."
  exit 0
else
  echo "One or more listeners failed readiness checks."
  exit 1
fi
