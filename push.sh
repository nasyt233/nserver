#!/bin/sh
set -e

VERSION="$1"
if [ -z "$VERSION" ]; then
    echo "用法: $0 <版本号>"
    exit 1
fi

echo "正在更新lock"
# 更新 lockfile
cargo generate-lockfile >/dev/null 2>&1 || cargo check >/dev/null 2>&1

# 删除旧tag
git tag -d "v$VERSION"
git push origin --delete "v$VERSION"

# 提交仓库
echo "正在提交仓库"
git add .
git commit -m "release: $VERSION"
git push origin main

echo "正在设置标签"
# 打 tag
git tag "v$VERSION"
git push origin "v$VERSION"

echo "已推送 tag v$VERSION"