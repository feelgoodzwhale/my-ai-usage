#!/usr/bin/env bash
set -euo pipefail

REPO_SLUG="feelgoodzwhale/my-ai-usage"
REPO_URL="https://github.com/${REPO_SLUG}.git"
APP_NAME="my-ai-usage"
APP_BUNDLE="${APP_NAME}.app"

FORCE_SOURCE=false
INSTALL_HOOK=true
UNINSTALL=false

# Colors
RED=$'\033[0;31m'
GREEN=$'\033[0;32m'
YELLOW=$'\033[1;33m'
CYAN=$'\033[0;36m'
BOLD=$'\033[1m'
NC=$'\033[0m'

info()  { printf "${CYAN}==>${NC} %s\n" "$*"; }
ok()    { printf "${GREEN}✓${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}!${NC} %s\n" "$*"; }
error() { printf "${RED}✗ Error:${NC} %s\n" "$*" >&2; exit 1; }

usage() {
  cat <<EOF
${BOLD}${APP_NAME} Installer for macOS${NC}

Usage:
  ./install.sh [options]
  curl -fsSL https://raw.githubusercontent.com/${REPO_SLUG}/main/install.sh | bash

Options:
  --from-source   Build from source even if a GitHub Release .dmg is available
  --no-hook       Skip configuring Claude / AGY statusLine hook
  --uninstall     Stop and remove ${APP_BUNDLE} from Applications
  -h, --help      Show this help message
EOF
}

for arg in "$@"; do
  case "$arg" in
    --from-source) FORCE_SOURCE=true ;;
    --no-hook)     INSTALL_HOOK=false ;;
    --uninstall)   UNINSTALL=true ;;
    -h|--help)     usage; exit 0 ;;
    *) error "Unknown option: $arg (use --help for usage)" ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  error "${APP_NAME} รองรับเฉพาะระบบปฏิบัติการ macOS เท่านั้น"
fi

choose_install_dir() {
  if [[ -w "/Applications" ]]; then
    echo "/Applications"
  else
    mkdir -p "$HOME/Applications"
    echo "$HOME/Applications"
  fi
}

do_uninstall() {
  info "กำลังปิดโปรเซส ${APP_NAME} (ถ้าเปิดอยู่)..."
  pkill -x "${APP_NAME}" 2>/dev/null || true

  for dir in "/Applications" "$HOME/Applications"; do
    if [[ -d "${dir}/${APP_BUNDLE}" ]]; then
      rm -rf "${dir}/${APP_BUNDLE}"
      ok "ลบ ${dir}/${APP_BUNDLE} เรียบร้อยแล้ว"
    fi
  done

  for bak in "$HOME/.claude/statusline-command.sh.my-ai-usage.bak" "$HOME/.claude/statusline-command.sh.my-usage.bak"; do
    if [[ -f "$bak" ]]; then
      mv -f "$bak" "$HOME/.claude/statusline-command.sh"
      chmod +x "$HOME/.claude/statusline-command.sh"
      ok "คืนค่า ~/.claude/statusline-command.sh เดิมเรียบร้อยแล้ว"
      break
    fi
  done

  ok "ถอนการติดตั้ง ${APP_NAME} เสร็จสมบูรณ์"
  exit 0
}

if [[ "$UNINSTALL" == true ]]; then
  do_uninstall
fi

install_app_bundle() {
  local src_app="$1"
  local dest_dir
  dest_dir="$(choose_install_dir)"
  local dest_app="${dest_dir}/${APP_BUNDLE}"

  info "กำลังปิด ${APP_NAME} เวอร์ชันเก่า (ถ้าเปิดรันอยู่)..."
  pkill -x "${APP_NAME}" 2>/dev/null || true
  sleep 0.4

  info "กำลังติดตั้งไปยัง ${dest_app}..."
  rm -rf "${dest_app}"
  cp -R "${src_app}" "${dest_app}"
  xattr -cr "${dest_app}" 2>/dev/null || true
  ok "ติดตั้งแอพที่ ${dest_app} เรียบร้อย"
}

try_install_from_github_release() {
  local arch
  arch="$(uname -m)"
  local arch_pattern="aarch64|arm64|universal"
  if [[ "$arch" == "x86_64" ]]; then
    arch_pattern="x86_64|x64|universal"
  fi

  info "กำลังตรวจสอบไฟล์ Release ล่าสุดบน GitHub..."
  local release_json
  release_json="$(curl -fsSL "https://api.github.com/repos/${REPO_SLUG}/releases/latest" 2>/dev/null || true)"
  if [[ -z "$release_json" ]]; then
    return 1
  fi

  local dmg_url
  dmg_url="$(printf '%s' "$release_json" | grep -Eo 'https://[^"]+\.dmg' | grep -Ei "$arch_pattern" | head -n 1 || true)"
  if [[ -z "$dmg_url" ]]; then
    dmg_url="$(printf '%s' "$release_json" | grep -Eo 'https://[^"]+\.dmg' | head -n 1 || true)"
  fi
  if [[ -z "$dmg_url" ]]; then
    return 1
  fi

  local tmp_dir
  tmp_dir="$(mktemp -d)"
  local dmg_file="${tmp_dir}/${APP_NAME}.dmg"
  local mount_dir="${tmp_dir}/mnt"
  mkdir -p "$mount_dir"

  info "กำลังดาวน์โหลด $(basename "$dmg_url")..."
  curl -fL --progress-bar "$dmg_url" -o "$dmg_file"

  info "กำลังเปิดอิมเมจ .dmg เพื่อคัดลอกแอพ..."
  hdiutil attach "$dmg_file" -nobrowse -quiet -mountpoint "$mount_dir"
  local cleanup_dmg="hdiutil detach \"$mount_dir\" -quiet 2>/dev/null || true; rm -rf \"$tmp_dir\""
  trap "$cleanup_dmg" EXIT

  if [[ ! -d "${mount_dir}/${APP_BUNDLE}" ]]; then
    eval "$cleanup_dmg"
    trap - EXIT
    return 1
  fi

  install_app_bundle "${mount_dir}/${APP_BUNDLE}"
  eval "$cleanup_dmg"
  trap - EXIT
  return 0
}

ensure_build_prerequisites() {
  if ! xcode-select -p >/dev/null 2>&1; then
    warn "ไม่พบ Xcode Command Line Tools กำลังเรียกตัวติดตั้ง..."
    xcode-select --install || true
    error "กรุณาติดตั้ง Xcode Command Line Tools ให้เสร็จก่อน แล้วรัน ./install.sh อีกครั้ง"
  fi

  if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
    if command -v brew >/dev/null 2>&1; then
      info "ไม่พบ Node.js กำลังติดตั้งผ่าน Homebrew..."
      brew install node
    else
      error "ไม่พบ Node.js/npm กรุณาติดตั้ง Node.js (https://nodejs.org) หรือ Homebrew ก่อน"
    fi
  fi

  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    info "ไม่พบ Rust (cargo) กำลังติดตั้งผ่าน rustup อัตโนมัติ..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
    ok "ติดตั้ง Rust สำเร็จ ($(rustc --version))"
  fi

  if ! command -v jq >/dev/null 2>&1 && command -v brew >/dev/null 2>&1; then
    info "กำลังติดตั้ง jq สำหรับอ่านข้อมูล statusLine..."
    brew install jq
  fi
}

build_and_install_from_dir() {
  local project_dir="$1"
  ensure_build_prerequisites

  info "กำลังติดตั้ง npm dependencies..."
  (cd "$project_dir" && npm install --no-fund --no-audit)

  info "กำลังคอมไพล์และสร้าง ${APP_BUNDLE} (อาจใช้เวลา 1-2 นาทีในครั้งแรก)..."
  (cd "$project_dir" && PATH="$HOME/.cargo/bin:$PATH" npm run tauri build -- --bundles app)

  local built_app="${project_dir}/src-tauri/target/release/bundle/macos/${APP_BUNDLE}"
  if [[ ! -d "$built_app" ]]; then
    error "ไม่พบไฟล์แอพที่สร้างเสร็จแล้วที่ ${built_app}"
  fi

  install_app_bundle "$built_app"

  if [[ "$INSTALL_HOOK" == true && -f "${project_dir}/scripts/install-statusline-hook.sh" ]]; then
    info "กำลังตั้งค่า statusLine hook สำหรับ Claude และ AGY..."
    bash "${project_dir}/scripts/install-statusline-hook.sh"
  fi
}

setup_standalone_hook_if_needed() {
  [[ "$INSTALL_HOOK" == true ]] || return 0
  command -v jq >/dev/null 2>&1 || return 0

  local tmp_hook_dir
  tmp_hook_dir="$(mktemp -d)"
  if curl -fsSL "https://raw.githubusercontent.com/${REPO_SLUG}/main/scripts/install-statusline-hook.sh" -o "${tmp_hook_dir}/hook.sh" 2>/dev/null; then
    info "กำลังตั้งค่า statusLine hook สำหรับ Claude และ AGY..."
    bash "${tmp_hook_dir}/hook.sh"
  fi
  rm -rf "$tmp_hook_dir"
}

# Detect if running directly inside a local checkout of the repository
SCRIPT_DIR=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

if [[ -n "$SCRIPT_DIR" && -f "${SCRIPT_DIR}/package.json" && -f "${SCRIPT_DIR}/src-tauri/tauri.conf.json" ]]; then
  info "ตรวจพบซอร์สโค้ดในโฟลเดอร์ปัจจุบัน (${SCRIPT_DIR})"
  # Reuse existing release build if already built and --from-source wasn't explicitly passed?
  # Building ensures latest local code is always installed cleanly.
  build_and_install_from_dir "$SCRIPT_DIR"
else
  INSTALLED_RELEASE=false
  if [[ "$FORCE_SOURCE" == false ]]; then
    if try_install_from_github_release; then
      INSTALLED_RELEASE=true
      setup_standalone_hook_if_needed
    else
      warn "ยังไม่มีไฟล์ .dmg บน GitHub Releases — จะทำการ Clone และ Build จากซอร์สโค้ดให้อัตโนมัติ"
    fi
  fi

  if [[ "$INSTALLED_RELEASE" == false ]]; then
    command -v git >/dev/null 2>&1 || error "ไม่พบคำสั่ง git กรุณาติดตั้ง git ก่อน"
    TMP_SRC="$(mktemp -d)"
    trap 'rm -rf "$TMP_SRC"' EXIT
    info "กำลังโคลนโปรเจกต์จาก ${REPO_URL}..."
    git clone --depth 1 "$REPO_URL" "$TMP_SRC/my-ai-usage"
    build_and_install_from_dir "$TMP_SRC/my-ai-usage"
    rm -rf "$TMP_SRC"
    trap - EXIT
  fi
fi

FINAL_DIR="$(choose_install_dir)"
info "กำลังเปิดแอพ ${FINAL_DIR}/${APP_BUNDLE} ขึ้นบน Menu Bar..."
open "${FINAL_DIR}/${APP_BUNDLE}"

printf "\n${GREEN}${BOLD}🎉 ติดตั้งและเปิดใช้งาน ${APP_NAME} สำเร็จแล้ว!${NC}\n"
printf "   มองที่แถบ Menu Bar ด้านบนขวาของจอเพื่อดูข้อมูล Claude & AGY ได้ทันที\n"
printf "   (หากต้องการถอนการติดตั้งในอนาคต รัน: ./install.sh --uninstall)\n"
