# NSERVER

**终端里的服务器管理面板**，用 Rust + ratatui 编写。左侧栏切换功能，右侧是内容区，目标是做出类似 BT 面板的日常运维体验，但全部运行在 SSH 终端里，不需要浏览器、不需要额外服务端。

```text
  ◆ NSERVER  v0.3.0   NGINX ON   DOCKER 0   FTP OFF        host  ·  load 0.12  ·  08:23:26
 ⌂ 首页   系统资源与服务概览                              Ctrl+B 侧栏  ·  Tab 页面
──────────────────────────────────────────────────────────────────────────────────────
╭ 导航 ────────────╮╭ CPU ──────────────────╮╭ 内存 ─────────────────╮╭ 磁盘 ────────╮
│ 1  ⌂ 首页        ││27.4%                  ││58%                    ││99%           │
│ 2  ◈ 站点        ││██████                 ││███████████ █          ││█████████████ │
│ 3  ▤ 文件        ││负载 0.12              ││6.4 GB / 11.1 GB       ││459.6/465.5GB │
│ 4  ▣ Docker      │╰───────────────────────╯╰───────────────────────╯╰──────────────╯
│ 5  ◔ 监控        │╭ 系统信息 ─────────────────────────╮╭ 服务与快捷操作 ──────────╮
│ 6  ≡ 日志        ││主机    localhost                  ││● Nginx   PID 30474      │
│ 7  ⛨ SSL         ││系统    Ubuntu 24.04.5 LTS         ││○ Docker  未安装         │
│ 8  ⇅ FTP         ││运行    3天 2时                     ││○ FTP     未安装 vsftpd  │
│ 9  ⚙ 设置        ││IP      10.0.0.12                  │╰─────────────────────────╯
╰──────────────────╯╰───────────────────────────────────╯
```

完整界面快照：[首页](docs/home.txt) · [站点](docs/sites.txt) · [文件](docs/files.txt) · [监控](docs/monitor.txt) · [日志](docs/logs.txt) · [SSL](docs/ssl.txt) · [设置](docs/settings.txt)。

## 功能

| 左侧栏 | 内容 |
| --- | --- |
| **首页** | CPU / 内存 / 磁盘 / 网络实时卡片、负载、运行时长、IP、服务状态、高占用进程、快捷操作 |
| **站点** | Nginx 站点发现、静态 / 反向代理 / 端口转发建站、域名端口、网站目录、SPA、WebSocket、代理目标、启停、回收站 |
| **文件** | 目录浏览、隐藏文件、文本预览、内置编辑器、新建目录/文件、重命名、删除到回收站 |
| **Docker** | 容器和镜像列表、启动 / 停止 / 重启、删除、容器日志、守护进程状态检测 |
| **监控** | CPU、内存、网络、磁盘趋势图，Swap、负载、运行时长、进程 Top 榜 |
| **日志** | Nginx 访问/错误日志 + 系统日志（syslog/messages）、筛选、跟随、自动刷新、横向滚动 |
| **SSL** | 读取 Nginx 引用的证书和 `ssl/` 目录证书、到期分级、详情、一键生成自签名证书 |
| **FTP** | vsftpd 检测、服务启停/重启/重载、配置摘要、系统用户列表、编辑 vsftpd.conf |
| **设置** | 主机信息、Nginx/FTP 路径、备份/回收站路径、主题、版本、快捷键 |
| **配置 / 上游** | Nginx 配置预览与编辑、upstream 后端 TCP 探测（通过 Tab 或站点页 `c`/`u` 进入） |

### 建站方式

在「站点」页按 `n`：

- **静态网站**：域名、监听端口、网站目录，可选 SPA 回退。
- **反向代理**：域名 → 目标地址，可选 WebSocket；生成独立升级 `map`，避免多站冲突。
- **端口转发**：域名填 `_`，把对外监听端口（例如 `8080`）反向代理到本机服务（例如 `127.0.0.1:3000`）。这是 HTTP 层转发，不是 iptables 四层转发。

选中站点后：

| 键 | 操作 |
| --- | --- |
| `m` | 网站设置（域名、端口、目录、代理、SPA、WebSocket） |
| `w` / `p` / `o` / `l` | 快速跳转到目录 / 代理 / 域名 / 端口 |
| `Space` | 启用 / 停用 |
| `e` | 完整配置编辑器 |
| `d` | 移入 `sites-available/.trash` |

快捷设置只修改第一个 `server` 块中的常见字段，保存前自动备份，其他自定义指令保持不变。

## 安装与运行

要求：Linux、Nginx、Rust **1.98+**（最新 stable 验证过）。Docker、vsftpd、openssl 都是可选的，对应页面会显示缺失提示。

```bash
cd /root/workspace/nserver
cargo build --release
sudo ./target/release/nserver
```

安装到 PATH：

```bash
cargo install --path .
sudo nserver
```

使用已有 nginx 配置：

```bash
sudo nserver -c /etc/nginx/nginx.conf
sudo nserver -c /srv/nginx/nginx.conf --bin /usr/sbin/nginx
```

只读模式 / 主题：

```bash
nserver --read-only --theme forest
```

环境变量：`NSERVER_CONFIG`（兼容 `NGINX_CONFIG`）、`NGINX_BIN`、`OPENSSL_BIN`。

### 一键演示

不修改 `/etc/nginx`，在 `/tmp` 创建独立配置、站点、日志和自签名证书，启动高位端口 Nginx 后打开面板；退出时自动停止 Nginx：

```bash
./scripts/demo.sh
# HTTP_PORT=28080 HTTPS_PORT=28443 ./scripts/demo.sh
```

演示包含静态站、HTTPS 站、停用站点、端口转发和 upstream。

## 快捷键

### 全局

| 按键 | 作用 |
| --- | --- |
| `1`–`9` | 首页 / 站点 / 文件 / Docker / 监控 / 日志 / SSL / FTP / 设置 |
| `Tab` / `Shift+Tab` | 在全部页面循环（含配置 / 上游） |
| `Ctrl+B` | 收起 / 展开左侧栏 |
| `?` | 帮助 |
| `q` / `Ctrl+C` | 退出（编辑器有修改会确认） |
| `T` | 切换主题 |
| `F5` / `Ctrl+R` | 刷新全部数据 |
| `t` / `r` / `s` / `R` | Nginx 校验 / 热重载 / 启停 / 重启 |

### 文件

`↑↓` 选择，`Enter` 进入目录或预览，`e` 编辑文本，`n` 新建目录，`t` 新建文件，`r` 重命名，`d` 移入 `~/.nserver/trash`，`o` 跳转路径，`Backspace` 返回上级，`.` 显示隐藏文件。

### Docker

`i` 切换容器/镜像，`s` 启动，`S` 停止，`r` 重启，`l` 日志，`d` 删除，`u` 刷新。

### FTP

`s` 启动，`S` 停止，`r` 重启，`l` 重载，`e` 编辑配置，`u` 刷新。

### 日志 / 监控 / SSL

日志：`f` 跟随、`a` 自动刷新、`L` 访问/错误/系统/全部、`[` `]` 换源、`h/l` 横向滚动、`u` 重读。

监控：`u` 立即重新采样。

SSL：`n` 生成自签名证书、`Enter` 查看详情。

## 安全设计

- **只读模式**：`--read-only` 在所有写入口拒绝操作。
- **写前备份**：配置和任意文件编辑前保存到 `.nserver/backups/`，使用原子替换。
- **删除可恢复**：站点删除到 `sites-available/.trash`，文件删除到 `~/.nserver/trash`。
- **危险操作确认**：停止/重启 Nginx、删除站点、删除文件、停止/删除容器等都会弹窗确认。
- **Nginx 先验后载**：`r` / `R` / 启动前先执行 `nginx -t`，失败时阻止后续动作。
- **只操作自己的配置**：站点启停只增删 `sites-enabled` 符号链接，并校验链接目标。
- **无 shell 拼接**：外部命令使用参数数组执行，不经过 `sh -c`。

## 代码结构

```text
src/
  main.rs       终端生命周期、事件循环
  cli.rs        参数、Nginx 路径探测
  app.rs        页面状态、键位、弹窗、编辑器、操作编排
  config.rs     Nginx 配置 tokenizer/parser、include 展开、站点/上游/日志提取
  service.rs    Nginx 进程、systemd/信号、命令输出
  sites.rs      站点发现、建站模板、局部字段 patch、启停/删除
  system.rs     /proc 与命令 fallback 的系统资源采样
  docker.rs     Docker CLI 容器/镜像
  ftp.rs        vsftpd 检测、配置、服务
  files.rs      文件浏览器、回收站、预览
  cert.rs       SSL 证书读取、自签名签发
  logs.rs       日志 tail 和状态/级别解析
  theme.rs      4 套主题
  ui/           左侧栏外壳与各页面
```

Nginx 配置解析不是完整语法实现，`nginx -t` 仍是唯一语法标准；自定义配置建议使用绝对 include / 日志 / 证书路径。

## 测试

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

当前 43 个测试，覆盖配置解析、建站模板、站点字段 patch、文件操作、日志解析、SSL 生成、系统资源采样、Docker/FTP 解析、编辑器备份，以及 64×20 / 80×24 / 120×40 三种尺寸下全部页面和弹窗的渲染测试。

已验证 Rust 1.98.0 和 1.100.0。

## 已知限制

- 这是 **TUI**，不是 Web 面板；需要通过 SSH/终端访问。
- Docker 页面只管理本机 Docker CLI 可见的容器/镜像，不含 Compose 编排。
- FTP 目前面向 vsftpd，使用系统用户，不在界面里创建系统账号。
- SSL 提供证书浏览和自签名签发，不内置 Let's Encrypt/ACME 自动续期。
- 文件编辑以运行用户权限执行，root 下操作需自行确认目标。
- 主要面向 Linux；资源采样依赖 `/proc`，缺失时使用命令 fallback。

## License

MIT
