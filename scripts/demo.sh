#!/usr/bin/env bash
# Start an isolated demo nginx on high ports and open nserver against it.
# Nothing under /etc/nginx is touched.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -f "$HOME/.cargo/env" ]]; then
    # Prefer the rustup toolchain over a possible old distro cargo.
    source "$HOME/.cargo/env"
fi

DEMO_DIR="${DEMO_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/nserver-demo.XXXXXX")}"
HTTP_PORT="${HTTP_PORT:-18080}"
HTTPS_PORT="${HTTPS_PORT:-18443}"
NGINX_BIN="${NGINX_BIN:-$(command -v nginx || true)}"
OPENSSL_BIN="${OPENSSL_BIN:-$(command -v openssl || true)}"
STARTED=0

if [[ -z "$NGINX_BIN" ]]; then
    echo "错误：没有找到 nginx。请先安装 nginx，或用 NGINX_BIN 指定路径。" >&2
    exit 1
fi
if [[ -z "$OPENSSL_BIN" ]]; then
    echo "错误：演示需要 openssl 生成自签名证书。" >&2
    exit 1
fi
if [[ -e "$DEMO_DIR/nginx.conf" ]]; then
    echo "错误：$DEMO_DIR 已经包含 nginx.conf，请换一个 DEMO_DIR。" >&2
    exit 1
fi

mkdir -p "$DEMO_DIR"/{logs,sites-available,sites-enabled,www,blog,certs}
echo '<h1>Hello from nserver</h1>' > "$DEMO_DIR/www/index.html"
echo '<h1>Blog (disabled)</h1>' > "$DEMO_DIR/blog/index.html"

cat > "$DEMO_DIR/nginx.conf" <<EOF
worker_processes 1;
pid $DEMO_DIR/nginx.pid;
error_log $DEMO_DIR/logs/error.log info;
events { worker_connections 128; }
http {
    access_log $DEMO_DIR/logs/access.log;
    upstream demo_app {
        least_conn;
        server 127.0.0.1:3000 weight=3;
        server 127.0.0.1:3001 backup;
    }
    include $DEMO_DIR/sites-enabled/*.conf;
}
EOF

cat > "$DEMO_DIR/sites-available/demo.conf" <<EOF
server {
    listen $HTTP_PORT;
    server_name demo.local www.demo.local;
    root $DEMO_DIR/www;
    index index.html;
    access_log $DEMO_DIR/logs/demo.access.log;
    error_log  $DEMO_DIR/logs/demo.error.log;
    location / { try_files \$uri \$uri/ =404; }
    location /api { proxy_pass http://demo_app; }
}
EOF

cat > "$DEMO_DIR/sites-available/secure.conf" <<EOF
server {
    listen $HTTPS_PORT ssl;
    server_name secure.demo.local;
    ssl_certificate     $DEMO_DIR/certs/demo.pem;
    ssl_certificate_key $DEMO_DIR/certs/demo.key;
    root $DEMO_DIR/www;
    access_log $DEMO_DIR/logs/secure.access.log;
    error_log  $DEMO_DIR/logs/secure.error.log;
    location / { try_files \$uri \$uri/ =404; }
}
EOF

cat > "$DEMO_DIR/sites-available/blog.conf" <<EOF
server {
    listen 18081;
    server_name blog.demo.local;
    root $DEMO_DIR/blog;
    index index.html;
    access_log $DEMO_DIR/logs/blog.access.log;
    error_log  $DEMO_DIR/logs/blog.error.log;
    location / { try_files \$uri \$uri/ =404; }
}
EOF

cat > "$DEMO_DIR/sites-available/forward.conf" <<EOF
server {
    listen 18082;
    server_name _;
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
EOF

ln -s ../sites-available/demo.conf "$DEMO_DIR/sites-enabled/demo.conf"
ln -s ../sites-available/secure.conf "$DEMO_DIR/sites-enabled/secure.conf"

"$OPENSSL_BIN" req -x509 -newkey rsa:2048 -nodes -days 30 \
    -subj '/CN=demo.local' \
    -addext 'subjectAltName=DNS:demo.local,DNS:www.demo.local' \
    -keyout "$DEMO_DIR/certs/demo.key" \
    -out "$DEMO_DIR/certs/demo.pem" >/dev/null 2>&1

cleanup() {
    if [[ "$STARTED" -eq 1 ]]; then
        "$NGINX_BIN" -s quit -c "$DEMO_DIR/nginx.conf" >/dev/null 2>&1 || true
        sleep 0.3
    fi
}
trap cleanup EXIT INT TERM

if ! "$NGINX_BIN" -t -c "$DEMO_DIR/nginx.conf"; then
    echo "演示配置校验失败，退出。" >&2
    exit 1
fi

if "$NGINX_BIN" -c "$DEMO_DIR/nginx.conf"; then
    STARTED=1
    sleep 0.3
    if command -v curl >/dev/null 2>&1; then
        for i in $(seq 1 8); do
            curl -s -o /dev/null "http://127.0.0.1:$HTTP_PORT/?i=$i" || true
        done
        curl -s -o /dev/null "http://127.0.0.1:$HTTP_PORT/api" || true
        curl -sk -o /dev/null "https://127.0.0.1:$HTTPS_PORT/" || true
    fi
    echo "演示 nginx 已启动：http://127.0.0.1:$HTTP_PORT  https://127.0.0.1:$HTTPS_PORT"
    echo "站点页还包含一个停用的 blog 和一个 18082→3000 的端口转发示例。"
    echo "演示目录：$DEMO_DIR"
    echo "退出 TUI 后会自动停止演示 nginx。"
    cargo run -- -c "$DEMO_DIR/nginx.conf" "$@"
else
    echo "警告：非 root 环境下演示 nginx 启动失败，将以只读模式打开界面。" >&2
    echo "演示目录：$DEMO_DIR"
    cargo run -- -c "$DEMO_DIR/nginx.conf" --read-only "$@"
fi
