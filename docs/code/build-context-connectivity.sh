#!/usr/bin/env bash
# Assemble docs/code/context-connectivity.txt — connectivity / "Updating…" problem:
# narrative, affected-file index, all project docs, chatmail-core + Madmail sources, tests.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT}/docs/code/context-connectivity.txt"
DOC="${ROOT}/docs/code"
CORE="${ROOT}/chatmail-core"

append_section() {
  local title="$1"
  local path="$2"
  {
    echo ""
    echo "================================================================================"
    echo "=== ${title}"
    echo "=== FILE: ${path#${ROOT}/}"
    echo "================================================================================"
    echo ""
    if [[ -f "$path" ]]; then
      cat "$path"
    else
      echo "(missing: $path)"
    fi
    echo ""
  } >>"$OUT"
}

append_raw() {
  local title="$1"
  {
    echo ""
    echo "================================================================================"
    echo "=== ${title}"
    echo "================================================================================"
    echo ""
  } >>"$OUT"
}

: >"$OUT"

{
  echo "MADMAIL — CONNECTIVITY / \"UPDATING…\" — FULL CONTEXT"
  echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "Repository: ${ROOT}"
  if command -v git >/dev/null 2>&1 && git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "Git branch: $(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
    echo "Git commit: $(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
  fi
  echo ""
  cat <<'NARRATIVE'
--------------------------------------------------------------------------------
OVERVIEW (read this first)
--------------------------------------------------------------------------------

The UI badge "Updating…" is NOT set by the Madmail Go server. It comes from
chatmail-core (madmail/chatmail-core/) when get_connectivity() returns
Connectivity::Working (3000–3999).

It ENDS when the inbox loop calls set_idle() after fetch_move_delete / downloads
and BEFORE IMAP IDLE. Until then, DetailedConnectivity is Preparing or Working.

Madmail affects this only via IMAP behavior: IDLE support, auto_logout vs
MinAutoLogout (30m), session drops, storage latency, NOTIFY on new mail.

KEY FILES:
  chatmail-core/src/scheduler/connectivity.rs  — state machine
  chatmail-core/src/scheduler.rs               — inbox_loop, fetch_idle, set_idle
  chatmail-core/src/imap.rs + src/imap/idle.rs — connect, fetch, IDLE
  internal/endpoint/imap/imap.go               — deadlineCapConn, auto_logout
  tests/imap_connection_lifecycle_test.go      — stuck-updating regression

CLIENT API (JSON-RPC / FFI):
  get_connectivity(account_id)
  get_connectivity_html(account_id)
  Event ConnectivityChanged → UI must re-query

STUCK "UPDATING…" CHECKLIST:
  1. fetch_idle failed before set_idle (IMAP error / timeout)
  2. Reconnect thrash (TCP/firewall/auto_logout misconfig)
  3. Long download backlog (Working until full pass done)
  4. Multi-folder: min() connectivity — one stuck folder blocks account
  5. UI not handling ConnectivityChanged (stale label)
  6. Do not confuse log "Updating quota." with UI string

Full file inventory: docs/code/connectivity-updating.md § Affected files inventory

NARRATIVE
} >>"$OUT"

# --- primary doc (includes inventory table) ---
append_section "DOCUMENTATION (primary)" "${DOC}/connectivity-updating.md"

# --- all developer docs/code ---
for f in "${DOC}"/*.md; do
  [[ "$f" == "${DOC}/connectivity-updating.md" ]] && continue
  append_section "DOCUMENTATION: docs/code" "$f"
done

# --- all project documentation ---
while IFS= read -r -d '' f; do
  append_section "DOCUMENTATION: docs" "$f"
done < <(find "${ROOT}/docs" -name '*.md' -type f ! -path '*/docs/code/*' -print0 | sort -z)

# --- chatmail-core: connectivity + scheduler + IMAP ---
CORE_SOURCES=(
  "${CORE}/src/scheduler/connectivity.rs"
  "${CORE}/src/scheduler.rs"
  "${CORE}/src/imap.rs"
  "${CORE}/src/imap/idle.rs"
  "${CORE}/src/imap/session.rs"
  "${CORE}/src/imap/select_folder.rs"
  "${CORE}/src/imap/client.rs"
  "${CORE}/src/imap/capabilities.rs"
  "${CORE}/src/imap/imap_tests.rs"
  "${CORE}/src/smtp.rs"
  "${CORE}/src/quota.rs"
  "${CORE}/src/stock_str.rs"
  "${CORE}/src/context.rs"
  "${CORE}/src/context/context_tests.rs"
  "${CORE}/src/accounts.rs"
  "${CORE}/src/config.rs"
  "${CORE}/src/configure.rs"
  "${CORE}/src/download.rs"
  "${CORE}/src/net.rs"
  "${CORE}/src/events/payload.rs"
  "${CORE}/src/receive_imf.rs"
  "${CORE}/src/imex/transfer.rs"
  "${CORE}/deltachat-jsonrpc/src/api.rs"
  "${CORE}/deltachat-jsonrpc/src/api/types/events.rs"
  "${CORE}/deltachat-ffi/src/lib.rs"
  "${CORE}/deltachat-ffi/deltachat.h"
  "${CORE}/deltachat-repl/src/cmdline.rs"
  "${CORE}/deltachat-repl/src/main.rs"
  "${CORE}/deltachat-rpc-server/src/main.rs"
)

for f in "${CORE_SOURCES[@]}"; do
  append_section "CHATMAIL-CORE SOURCE" "$f"
done

# --- Python / RPC bindings ---
CORE_BINDINGS=(
  "${CORE}/python/src/deltachat/account.py"
  "${CORE}/python/src/deltachat/events.py"
  "${CORE}/python/src/deltachat/testplugin.py"
  "${CORE}/python/tests/test_1_online.py"
  "${CORE}/python/tests/test_3_offline.py"
  "${CORE}/python/tests/test_4_lowlevel.py"
  "${CORE}/python/tests/test_0_complex_or_slow.py"
  "${CORE}/deltachat-rpc-client/src/deltachat_rpc_client/account.py"
  "${CORE}/deltachat-rpc-client/src/deltachat_rpc_client/deltachat.py"
  "${CORE}/deltachat-rpc-client/src/deltachat_rpc_client/const.py"
  "${CORE}/deltachat-rpc-client/src/deltachat_rpc_client/rpc.py"
  "${CORE}/deltachat-rpc-client/src/deltachat_rpc_client/client.py"
  "${CORE}/deltachat-rpc-client/src/deltachat_rpc_client/pytestplugin.py"
  "${CORE}/deltachat-rpc-client/tests/test_multitransport.py"
  "${CORE}/deltachat-rpc-client/tests/test_something.py"
  "${CORE}/deltachat-rpc-client/tests/test_folders.py"
  "${CORE}/deltachat-jsonrpc/typescript/test/online.ts"
)

for f in "${CORE_BINDINGS[@]}"; do
  append_section "CHATMAIL-CORE BINDINGS/TESTS" "$f"
done

# --- Madmail server + tests ---
MADMAIL_SOURCES=(
  "${ROOT}/internal/endpoint/imap/imap.go"
  "${ROOT}/internal/go-imap-sql/delivery.go"
  "${ROOT}/internal/go-imap-sql/backend.go"
  "${ROOT}/internal/go-imap-sql/mailbox.go"
  "${ROOT}/internal/go-imap-sql/user.go"
  "${ROOT}/internal/go-imap-sql/flags.go"
  "${ROOT}/internal/go-imap-mess/mailbox.go"
  "${ROOT}/internal/go-imap-mess/sequpdate.go"
  "${ROOT}/framework/hooks/hooks.go"
  "${ROOT}/internal/cli/ctl/install.go"
  "${ROOT}/tests/imap_connection_lifecycle_test.go"
  "${ROOT}/tests/test-client/main.go"
  "${ROOT}/scripts/goroutine_idle_experiment.sh"
  "${ROOT}/tests/deltachat-test/scenarios/test_12_smtp_imap_idle.py"
  "${ROOT}/tests/deltachat-test/scenarios/test_01_account_creation.py"
  "${ROOT}/tests/deltachat-test/scenarios/test_13_concurrent_profiles.py"
  "${ROOT}/tests/deltachat-test/scenarios/test_07_federation.py"
  "${ROOT}/tests/deltachat-test/main.py"
  "${ROOT}/tests/deltachat-test/cmping.py"
)

for f in "${MADMAIL_SOURCES[@]}"; do
  append_section "MADMAIL SOURCE/TEST" "$f"
done

# --- config excerpts ---
append_raw "CONFIG EXCERPT: maddy.conf (imap / storage.imapsql lines)"
if [[ -f "${ROOT}/maddy.conf" ]]; then
  grep -n -E '^(imap |storage\.imapsql|auto_logout)' "${ROOT}/maddy.conf" >>"$OUT" 2>/dev/null || true
  echo "" >>"$OUT"
  echo "(full file: maddy.conf)" >>"$OUT"
fi

append_raw "CONFIG EXCERPT: maddy.conf.j2 (imap / auto_logout)"
if [[ -f "${ROOT}/internal/cli/ctl/maddy.conf.j2" ]]; then
  grep -n -E 'imap |auto_logout|imapsql' "${ROOT}/internal/cli/ctl/maddy.conf.j2" >>"$OUT" 2>/dev/null || true
  echo "" >>"$OUT"
  echo "(full file: internal/cli/ctl/maddy.conf.j2)" >>"$OUT"
fi

# --- optional test output ---
append_raw "TEST OUTPUT (go test imap lifecycle — optional)"
{
  echo "\$ cd ${ROOT} && go test ./tests/ -run TestIMAP_AutoLogoutBelowMin -count=1 -v"
  (cd "$ROOT" && go test ./tests/ -run TestIMAP_AutoLogoutBelowMin -count=1 -v) 2>&1 || true
} >>"$OUT"

{
  echo ""
  echo "================================================================================"
  echo "=== END OF context-connectivity.txt"
  echo "=== Regenerate: bash docs/code/build-context-connectivity.sh"
  echo "=== Index: docs/code/connectivity-updating.md"
  echo "================================================================================"
} >>"$OUT"

wc -l "$OUT" | awk -v f="$OUT" '{print "Wrote " $1 " lines to " f}'
ls -lh "$OUT"
