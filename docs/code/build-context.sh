#!/usr/bin/env bash
# Assemble madmail/docs/code/context.txt — single file with explanation,
# related docs, source, integration points, and benchmark output.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${ROOT}/docs/code/context.txt"
DOC="${ROOT}/docs/code"
PGP="${ROOT}/internal/pgp_verify"

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
  echo "MADMAIL — MESSAGE CHECKS, PGP POLICY, PIPELINE, BENCHMARKS — FULL CONTEXT"
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

Madmail enforces a chatmail-style PGP-only policy: every accepted message must
either be RFC 3156 multipart/encrypted (OpenPGP packet framing validated, not
decrypted), an allowed Secure-Join handshake, a configured passthrough, or a
strict mailer-daemon DSN bounce.

CENTRAL PACKAGE: internal/pgp_verify/

  policy.go       — Policy, EnforcePolicy, StrictSubmissionPolicy, envelope rules
  pgp_verify.go   — MIME + OpenPGP streaming walker (walkOpenPGPPackets, armor)
  metrics.go      — MeasureEnforceEncryption / MeasureEnforcePolicy (RunStats)
  *_test.go       — unit, adversarial, measure, benchmark tests

PUBLIC API (use these, not duplicate checks):

  EnforcePolicy(header, body, policy)     — submission + unified policy
  EnforceEncryption(header, body, opts)   — opts → PolicyFromOptions
  MeasureEnforceEncryption(...)           — timing + alloc stats per call

Rejection: SMTP 523 / 5.7.1 "Encryption Needed: Invalid Unencrypted Mail"
(singleton errRejectUnencrypted — no per-reject alloc).

DECISION ORDER (EnforcePolicy / EnforceEncryption):

  1. Passthrough sender (exact MailFrom match)
  2. Passthrough recipients (every RCPT matches list or @domain)
  3. Mailer-daemon bounce (envelope + Auto-Submitted + multipart/report + From)
  4. Content-Type multipart/encrypted → streamValidateEncryptedMIME
  5. Content-Type multipart/mixed + Secure-Join header → streamValidateSecureJoinMIME
  6. Else → reject without reading body (cleartext fast path)

RFC 3156 VALIDATION (streamValidateEncryptedMIME):

  - Exactly 2 MIME parts
  - Part 1: application/pgp-encrypted, body "Version: 1"
  - Part 2: application/octet-stream → streamValidateOpenPGPPayload
      * ASCII armor: strip headers/footer, stream base64 decode
      * Binary: walk packets directly
  - walkOpenPGPPackets: zero+ PKESK(1)/SKESK(3), then one SEIPD(18) to EOF
  - Extra bytes after SEIPD → reject (prevents trailing cleartext in same part)

SUBMISSION PIPELINE (why CPU spikes on ~30 MiB uploads):

  Client DATA → prepareBody (full message buffered to RAM or state_dir/buffer/)
             → submissionCheckBody → pgp_verify.EnforcePolicy (full read #1)
             → msgpipeline → check.pgp_encryption (skipped if PGPPolicyVerified)
             → queue/mailbox (often another full read/write)

Install template: PGP on submission endpoint (pgp_allow_secure_join,
pgp_passthrough_*). Do NOT also use check { pgp_encryption { } } on submission
(duplicate scan). Migrate old configs: madmail migrate-pgp-config

UNIFIED SUBMISSION CHECK:

  internal/endpoint/smtp/submission.go — submissionCheckBody calls EnforcePolicy,
  sets MsgMetadata.PGPPolicyVerified = true

  internal/check/pgp_encryption/pgp_encryption.go — skips body when flag set

CONFIG MIGRATION:

  internal/confutil/migrate_submission_pgp.go
  internal/cli/ctl/migrate_pgp_config.go — madmail migrate-pgp-config
  chatmail/reload.go — runs migration when writing pending config

BENCHMARKS & OPTIMIZATION (internal/pgp_verify/)

  Tests:
    TestMakeBinaryPGP_Validates     — binary multipart fixture regression
    TestMeasureEnforceEncryption_Iterations — 1/5/30 MiB with RunStats
    BenchmarkEnforceEncryption_*    — armored/binary/cleartext reject

  Run:
    go test ./internal/pgp_verify/ -bench=BenchmarkEnforceEncryption -benchmem -run=^$
    go test ./internal/pgp_verify/ -run TestMeasureEnforceEncryption_Iterations -v

  Fixture fix (May 2026):
    Manual MIME assembly glued "--boundary--" into part 2 without leading CRLF,
    so binary OpenPGP validation failed (armored still passed: armor reader stops
    at -----END PGP MESSAGE----- before junk). Fixtures now use mime/multipart.Writer
    (writeEncryptedMIME in bench_test.go).

  Code optimizations:
    - readOpenPGPBodyLen extracted from walkOpenPGPPackets
    - 64 KiB bufio on OpenPGP hot path
    - armorReader.Read fills caller buffer across multiple armor lines per Read
      (~19% faster armored 5 MiB benchmark on dev hardware)

  Typical results (i7-1370P, may vary):
    Armored 5 MiB:  ~8–10 ms/op, ~41 allocs/op
    Binary 5 MiB:   ~0.7 ms/op,  ~31 allocs/op (no base64)
    Armored 100 MiB: ~200 ms (measure test)
    Cleartext reject: ~150 ns/op, body not read

ARCHITECTURE AUDIT (message-checks-pipeline.md):

  Submission duplicate PGP scan: SOLVED via PGPPolicyVerified + migrate-pgp-config.
  Socket → buffer → EnforcePolicy → msgpipeline: working as designed on submission.

  Remaining inconsistencies:
    - LMTP require_pgp: does not set PGPPolicyVerified (pipeline may re-scan)
    - WebSMTP: no PGPPolicyVerified on msgMeta if target uses msgpipeline + pgp_encryption
    - IMAP APPEND: io.ReadAll entire message (RAM spike on large APPEND)
    - Exchanger inject: no pgp_verify (by design)

  Optimization status:
    B1/B2 PGPPolicyVerified parity (LMTP, WebSMTP) — done
    B3 queue hardlink (FileBuffer.LinkAt) — done
    B4 IMAP APPEND buffer.SpillReader — done

RELATED DOCS IN THIS REPO (also embedded below):

  docs/code/message-checks-pipeline.md — checks vs pipeline audit (START HERE)
  docs/code/pgp-verification.md  — policy + call sites
  docs/code/performance.md       — SMTP DATA pipeline + I/O
  docs/code/message-incoming.md / message-outgoing.md — mail paths
  docs/code/chatmail.md          — chatmail integration
  docs/chatmail/only_pgp_mails.md — user-facing policy

KEY INTEGRATION FILES (embedded below):

  internal/endpoint/smtp/submission.go
  internal/endpoint/smtp/session.go
  internal/msgpipeline/msgpipeline.go
  internal/check/pgp_encryption/pgp_encryption.go
  internal/confutil/migrate_submission_pgp.go
  internal/cli/ctl/migrate_pgp_config.go
  internal/endpoint/chatmail/chatmail.go (mxdeliv excerpt via full file)
  internal/endpoint/webimap/websmtp.go
  framework/module/msgmetadata.go (PGPPolicyVerified)

NARRATIVE
} >>"$OUT"

# --- documentation ---
for f in \
  "${DOC}/message-checks-pipeline.md" \
  "${DOC}/pgp-verification.md" \
  "${DOC}/performance.md" \
  "${DOC}/architecture.md" \
  "${DOC}/chatmail.md" \
  "${ROOT}/docs/chatmail/only_pgp_mails.md" \
  "${DOC}/message-outgoing.md" \
  "${DOC}/message-incoming.md" \
  "${DOC}/README.md"
do
  append_section "DOCUMENTATION" "$f"
done

# --- pgp_verify package (full) ---
for f in "${PGP}"/*.go; do
  append_section "SOURCE: pgp_verify" "$f"
done

# --- integration ---
for f in \
  "${ROOT}/internal/endpoint/smtp/submission.go" \
  "${ROOT}/internal/endpoint/smtp/session.go" \
  "${ROOT}/internal/msgpipeline/msgpipeline.go" \
  "${ROOT}/internal/check/pgp_encryption/pgp_encryption.go" \
  "${ROOT}/internal/endpoint/webimap/websmtp.go" \
  "${ROOT}/internal/confutil/migrate_submission_pgp.go" \
  "${ROOT}/internal/cli/ctl/migrate_pgp_config.go" \
  "${ROOT}/framework/module/msgmetadata.go"
do
  append_section "INTEGRATION" "$f"
done

# --- install template excerpt (pgp directives) ---
append_raw "INSTALL TEMPLATE (pgp_* directives in maddy.conf.j2)"
if [[ -f "${ROOT}/internal/cli/ctl/maddy.conf.j2" ]]; then
  grep -n -E 'pgp_|pgp_encryption|submission' "${ROOT}/internal/cli/ctl/maddy.conf.j2" >>"$OUT" 2>/dev/null || true
  echo "" >>"$OUT"
  echo "(full file: internal/cli/ctl/maddy.conf.j2)" >>"$OUT"
fi

# --- benchmarks (live output) ---
append_raw "BENCHMARK OUTPUT (go test ./internal/pgp_verify/ …)"
{
  echo "\$ cd ${ROOT} && go test ./internal/pgp_verify/ -count=1"
  (cd "$ROOT" && go test ./internal/pgp_verify/ -count=1) 2>&1 || true
  echo ""
  echo "\$ go test ./internal/pgp_verify/ -bench=BenchmarkEnforceEncryption -benchmem -run=^\$"
  (cd "$ROOT" && go test ./internal/pgp_verify/ -bench=BenchmarkEnforceEncryption -benchmem -run='^$' -count=1) 2>&1 || true
  echo ""
  echo "\$ go test ./internal/pgp_verify/ -run TestMeasureEnforceEncryption_Iterations -v"
  (cd "$ROOT" && go test ./internal/pgp_verify/ -run TestMeasureEnforceEncryption_Iterations -v -count=1) 2>&1 || true
} >>"$OUT"

# --- footer ---
{
  echo ""
  echo "================================================================================"
  echo "=== END OF context.txt"
  echo "=== Regenerate: bash docs/code/build-context.sh"
  echo "================================================================================"
} >>"$OUT"

wc -l "$OUT" | awk '{print "Wrote " $1 " lines to " FILENAME}' FILENAME="$OUT"
ls -lh "$OUT"
