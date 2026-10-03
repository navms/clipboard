#!/usr/bin/env bash
#
# Rebuild -> verify signature -> install to /Applications -> launch.
#
# Why the signature check matters (and why "the build succeeded" is not enough):
#
#   The Accessibility (TCC) grant is matched against the app's *designated
#   requirement*. An adhoc signature produces a DR made of nothing but a
#   CDHash, which changes on every recompile -- the system then sees a
#   different app and the grant lapses. A signature from a stable identity
#   produces a DR of "bundle id + certificate", independent of the binary's
#   contents, so rebuilds keep the grant.
#
#   This script therefore refuses to install an adhoc-signed bundle. Shipping
#   one is the worst kind of bug: it works until the next rebuild, so it reads
#   as "it broke by itself".
#
# Usage:
#   ./scripts/reinstall.sh                # build, verify signature, install
#   ./scripts/reinstall.sh --allow-adhoc  # skip the signature check
#
# Signing identity
# ----------------
#   src-tauri/tauri.conf.json ships with an empty `bundle.macOS` object on
#   purpose: a hardcoded certificate identity would leak a personal email and
#   Team ID, and would break the build for anyone who forks this repo. Supply
#   the identity from the environment instead -- the Tauri CLI reads
#   APPLE_SIGNING_IDENTITY:
#
#   export APPLE_SIGNING_IDENTITY="Apple Development: you@example.com (TEAMID)"
#   ./scripts/reinstall.sh
#
#   For a release build, let CI inject the certificate (see
#   .github/workflows/release.yml).
#
#   With no identity set you get an adhoc-signed bundle, which is fine for
#   poking at a build but will lose the Accessibility grant on the next
#   recompile. That is what --allow-adhoc is for.

set -euo pipefail

APP_NAME="Clipboard History"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE="${ROOT}/src-tauri/target/release/bundle/macos/${APP_NAME}.app"
TARGET="/Applications/${APP_NAME}.app"
LOG="/tmp/clipboard-reinstall.log"
LSR="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"

ALLOW_ADHOC=0
for arg in "$@"; do
  case "${arg}" in
    --allow-adhoc) ALLOW_ADHOC=1 ;;
    -h|--help)
      sed -n '2,36p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown argument: ${arg} (try --help)" >&2
      exit 2
      ;;
  esac
done

cd "${ROOT}"
export PATH="${HOME}/.cargo/bin:${PATH}"

echo "==> Building"
if [ "${ALLOW_ADHOC}" -eq 0 ] && [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "   APPLE_SIGNING_IDENTITY is not set, so this build will be adhoc-signed"
  echo "   and the Accessibility grant will lapse on the next rebuild."
  echo "   Set it, or pass --allow-adhoc if that is fine."
fi
pnpm tauri build --bundles app

echo
echo "==> Verifying signature"
if [ ! -d "${BUNDLE}" ]; then
  echo "   FAIL no bundle at ${BUNDLE}" >&2
  exit 1
fi

DR="$(codesign -d -r- "${BUNDLE}" 2>&1 | grep designated || true)"
if [ -z "${DR}" ]; then
  echo "   FAIL could not read the designated requirement" >&2
  exit 1
fi
echo "   ${DR:0:104}"

if printf '%s' "${DR}" | grep -q cdhash; then
  if [ "${ALLOW_ADHOC}" -eq 1 ]; then
    echo "   WARN adhoc signature, allowed by --allow-adhoc"
    echo "        The Accessibility grant will lapse on the next rebuild."
  else
    cat >&2 <<'EOF'

   FAIL the bundle is still adhoc-signed -- every rebuild will drop the
        Accessibility grant. Check:
          1. APPLE_SIGNING_IDENTITY is exported and matches
               security find-identity -v -p codesigning
             exactly, Team ID included
          2. the certificate is still valid
          3. for a release build, the repository secrets are set
               (see .github/workflows/release.yml)

   Or re-run with --allow-adhoc if you are just poking at a local build.
EOF
    exit 1
  fi
else
  echo "   OK  DR has no cdhash -- rebuilds leave existing grants intact"
fi

echo
echo "==> Installing"
pkill -f "${TARGET}" 2>/dev/null || true
sleep 1
rm -rf "${TARGET}"
cp -R "${BUNDLE}" /Applications/
xattr -dr com.apple.quarantine "${TARGET}" 2>/dev/null || true
"${LSR}" -f "${TARGET}"
echo "   OK  installed to ${TARGET}"

echo
echo "==> Launching and reading the permission state"
# Running the binary directly (rather than `open -a`) is what lets us capture
# stdout -- the permission state is only reported there.
nohup "${TARGET}/Contents/MacOS/clipboard" >"${LOG}" 2>&1 &
sleep 4

if ! pgrep -f "${TARGET}/Contents/MacOS/" >/dev/null; then
  echo "   FAIL process did not survive; log follows:" >&2
  cat "${LOG}" >&2
  exit 1
fi

if grep -q "armed" "${LOG}"; then
  echo "   OK  double-tap Option is ready (Accessibility granted)"
elif grep -q "inert" "${LOG}"; then
  echo "   NOTE Accessibility not granted yet -- double-tap Option and"
  echo "        auto-paste stay disabled."
  echo "        System Settings > Privacy & Security > Accessibility, then tick"
  echo "        \"${APP_NAME}\". Granting it takes effect without a restart."
else
  echo "   NOTE could not determine the permission state; see ${LOG}"
fi

echo
echo "Done."
