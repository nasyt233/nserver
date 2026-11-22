#!/bin/sh
set -e

# NSERVER release installer. Override these variables when using a mirror.
APP_NAME="nserver"
GITCODE_REPO="nasyt/nserver"
GITHUB_REPO="nasyt233/nserver"
GITCODE_API="https://api.gitcode.com/api/v5/repos/${GITCODE_REPO}/releases/latest"
GITCODE_DL="https://gitcode.com/${GITCODE_REPO}/releases/download"
GITHUB_API="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
GITHUB_DL="https://github.com/${GITHUB_REPO}/releases/download"
TAG_PREFIX="${APP_NAME}-"

RED=$(printf '\033[0;31m')
GREEN=$(printf '\033[0;32m')
YELLOW=$(printf '\033[1;33m')
CYAN=$(printf '\033[0;36m')
NC=$(printf '\033[0m')

info() { printf '%s[%s]%s %s\n' "$CYAN" "$APP_NAME" "$NC" "$*"; }
ok() { printf '%s[%s]%s %s\n' "$GREEN" "$APP_NAME" "$NC" "$*"; }
warn() { printf '%s[%s]%s %s\n' "$YELLOW" "$APP_NAME" "$NC" "$*"; }
err() { printf '%s[%s]%s %s\n' "$RED" "$APP_NAME" "$NC" "$*" >&2; }

require_commands() {
    missing=""
    for command_name in curl grep sed head uname mktemp chmod mv mkdir; do
        if ! command -v "$command_name" >/dev/null 2>&1; then
            missing="$missing $command_name"
        fi
    done
    if [ -n "$missing" ]; then
        err "缺少命令:$missing"
        exit 1
    fi
}

detect_platform() {
    OS_RAW=$(uname -s 2>/dev/null || echo unknown)
    ARCH_RAW=$(uname -m 2>/dev/null || echo unknown)
    case "$OS_RAW" in
        Linux*) OS=linux ;;
        Darwin*) OS=macos ;;
        MINGW*|MSYS*|CYGWIN*) OS=win ;;
        *) err "不支持的系统: $OS_RAW"; exit 1 ;;
    esac
    case "$ARCH_RAW" in
        x86_64|amd64) ARCH=amd64 ;;
        aarch64|arm64) ARCH=arm64 ;;
        armv7l|armv7*) ARCH=armv7 ;;
        i?86) ARCH=i686 ;;
        riscv64) ARCH=riscv64 ;;
        ppc64le) ARCH=ppc64le ;;
        *) err "不支持的架构: $ARCH_RAW"; exit 1 ;;
    esac
    # Termux packages are Linux binaries even though uname may report Android.
    if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX" ]; then
        OS=linux
    fi
    [ "$OS" = "win" ] && EXT=".exe" || EXT=""
}

get_latest_tag() {
    for api in "$GITCODE_API" "$GITHUB_API"; do
        tag=$(curl -fsSL --max-time 15 "$api" 2>/dev/null \
            | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' \
            | sed -n '1{s/.*"\([^"]*\)"$/\1/p;}') || tag=""
        case "$tag" in
            "$TAG_PREFIX"*) printf '%s' "$tag"; return 0 ;;
            *) [ -n "$tag" ] && warn "忽略不匹配的版本标签: $tag" >&2 ;;
        esac
    done
    return 1
}

choose_install_dir() {
    if [ -n "${PREFIX:-}" ] && [ -d "$PREFIX/bin" ]; then
        bin_dir="$PREFIX/bin"
    elif [ -w /usr/local/bin ]; then
        bin_dir="/usr/local/bin"
    elif command -v sudo >/dev/null 2>&1; then
        bin_dir="/usr/local/bin"
        need_sudo=1
    else
        bin_dir="${HOME:-.}/.local/bin"
        mkdir -p "$bin_dir"
    fi
    target="$bin_dir/$APP_NAME"
}

main() {
    require_commands
    detect_platform
    choose_install_dir
    info "检测到平台: ${OS}-${ARCH}${EXT}"
    info "正在获取最新版本信息..."
    tag_name=$(get_latest_tag) || { err "获取版本信息失败，GitCode 和 GitHub 均不可用或没有匹配版本"; exit 1; }
    version=${tag_name#"$TAG_PREFIX"}
    bin_name="${APP_NAME}-${version}-${OS}-${ARCH}${EXT}"
    info "最新版本: $version"
    info "安装到: $target"

    tmp=$(mktemp "${TMPDIR:-/tmp}/${APP_NAME}.XXXXXX")
    cleanup() { rm -f "$tmp" 2>/dev/null || true; }
    trap cleanup EXIT INT TERM

    downloaded=0
    for url in \
        "${GITCODE_DL}/${tag_name}/${bin_name}" \
        "${GITHUB_DL}/${tag_name}/${bin_name}"; do
        info "下载: $url"
        if curl --progress-bar -f -L --max-time 300 -o "$tmp" "$url"; then
            downloaded=1
            break
        fi
        warn "源失败，尝试下一个"
    done
    if [ "$downloaded" -ne 1 ] || [ ! -s "$tmp" ]; then
        err "所有下载源均失败"
        err "请手动下载: https://github.com/${GITHUB_REPO}/releases"
        exit 1
    fi

    chmod +x "$tmp"
    if [ "${need_sudo:-0}" = "1" ] && [ ! -w "$bin_dir" ]; then
        sudo mkdir -p "$bin_dir"
        sudo mv "$tmp" "$target"
    else
        mkdir -p "$bin_dir"
        mv "$tmp" "$target"
    fi

    ok "$APP_NAME 已安装: $target (v$version)"
    case ":${PATH:-}:" in
        *":$bin_dir:"*) ;;
        *) warn "$bin_dir 不在 PATH 中，请加入 ~/.profile 或 ~/.bashrc" ;;
    esac
    info "运行 '$APP_NAME' 启动"
}

main "$@"
