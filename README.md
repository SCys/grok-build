# Grok Build（国内可用分支）

这是 [SpaceXAI Grok Build](https://x.ai/cli) 的本地分支，目标是在**国内网络**下也能启动、编译，并用第三方 OpenAI 兼容接口跑起来。

上游源码仍在 `main`（定期从 SpaceXAI 单仓同步）。本地改动按功能拆分支，方便 rebase 上游、也方便单独向上游提 PR。

**官方产品说明仍以 [x.ai/cli](https://x.ai/cli) 和 [docs.x.ai/build](https://docs.x.ai/build/overview) 为准。** 本仓库多出来的，只是国内网络下的启动门闸、默认不自动升级，以及 Docker 编译。

---

## 这个分支改了什么

国内访问 `auth.x.ai`、`cli-chat-proxy.grok.com` 经常不通。官方启动会：自动打开浏览器登录、拉取账号 settings / 模型目录、后台检查升级。不通时会卡到几十秒然后报 `startup timed out`。

本分支的策略是：**先探测关键域名，不通就跳过这些网络步骤；通了则走官方逻辑。**

| 探测目标 | 不通时跳过 | 通了时 |
| --- | --- | --- |
| `https://auth.x.ai` | 启动自动登录、启动 token 刷新 | 和官方一样弹出 / 刷新登录 |
| `https://cli-chat-proxy.grok.com/v1` | 远程 settings、模型目录 prefetch、启动升级检查 | 和官方一样拉取 |

探测是约 500ms 的 HTTPS GET，会走环境变量里的 `https_proxy` / `HTTPS_PROXY`（不是裸 TCP）。任意 HTTP 响应（含 401/404）都算通。

另外：

- `[cli] auto_update` 未设置时默认 **关闭**，避免源码构建被官方安装包覆盖。需要时设 `auto_update = true` 或运行 `grok update`。
- 仍可用 `/login` 或 `grok --force-login` 手动登录。
- 有代理时设上 `https_proxy`，官方 grok.com 通路会自动恢复。

功能分支：

| 分支 | 内容 |
| --- | --- |
| `main` | 上游同步点，不放本地改动 |
| `local/startup-connectivity-gate` | 连通性门闸 |
| `local/no-startup-auto-update` | 默认不自动升级 |
| `local/docker-build` | 国内镜像 Docker 编译 |
| `local/github-actions-ci` | GitHub Actions 测试与构建 |
| `local-main` | 上面几项的合集，日常使用 |

---

## 对接第三方模型

Grok Build 支持在 `~/.grok/config.toml` 里接 **OpenAI Chat Completions 兼容**网关（也支持 `responses`、Anthropic `messages`）。国内常用做法：用可访问的 LLM 网关，把默认模型和 `web_search` 都指过去，启动时即使 grok.com 不通也能对话。

密钥不要写进仓库。推荐用环境变量：

```sh
export MY_LLM_API_KEY="sk-..."
```

```toml
# ~/.grok/config.toml

[cli]
auto_update = false

[models]
default = "gemini-flash"          # 新会话默认模型（config 里的 [model.<name>]）
default_reasoning_effort = "high"
web_search = "gemini-flash"       # web_search 工具用的模型，同样走第三方

[model.gemini-flash]
model = "gemini-3.7-flash"        # 发给上游 API 的模型 ID
base_url = "https://your-llm-gateway.example/v1"
api_backend = "chat_completions"  # OpenAI 兼容 /v1/chat/completions
name = "Gemini 3.7 Flash"         # 模型选择器里显示的名字
env_key = "MY_LLM_API_KEY"        # 从环境变量读密钥，不要把 key 写进文件
supports_reasoning_effort = true
```

要点：

- `[model.<name>]` 的 `<name>` 是 Grok 内部的配置键；`model = "..."` 才是请求体里的模型 ID。
- `base_url` 填网关的 OpenAI 兼容根路径，一般以 `/v1` 结尾。
- `api_backend`：`chat_completions`（默认）、`responses`、`messages`。
- 密钥优先级：该模型的 `api_key` → `env_key` 环境变量 → 已登录的 grok.com session → 全局 `XAI_API_KEY`。第三方网关请用前两项，不要依赖 grok.com 登录。
- `web_search` 也要指到你能访问的模型，否则搜索工具仍会打官方模型。

需要走官方 Grok 模型时：

```sh
export https_proxy=http://127.0.0.1:7890   # 你的代理
grok login
# 把 [models] default 改回 grok-4.6 等官方 ID
```

更完整的字段说明见 [Custom Models](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md)。

---

## 用 Docker 编译并替换本机 grok

本机可以不装 Rust。脚本会用国内镜像编译，并替换 `~/.grok/bin/grok`（`~/.local/bin/grok` 一般已指向这里）。

镜像：apt → 清华，rustup / crates.io → rsproxy，GitHub → ghfast.top。宿主机的 `https_proxy` 会传进构建。

```sh
git checkout local-main
./scripts/docker-build-install.sh              # 编译 + 安装
./scripts/docker-build-install.sh --build-only # 只编译到 dist/grok
./scripts/docker-build-install.sh --install-only
```

官方安装包会留在 `~/.grok/downloads/` 里作备份。安装脚本会把 `auto_update` 写成 `false`，避免下次启动被官方包盖掉。

本机有 Rust 时也可以：

```sh
cargo build -p xai-grok-pager-bin --release
# 产物：target/release/xai-grok-pager
```

---

## 上游：安装发行版 / 从源码构建

发行版二进制（官方安装渠道，国内可能拉不下）：

```sh
curl -fsSL https://x.ai/cli/install.sh | bash   # macOS / Linux / Git Bash
irm https://x.ai/cli/install.ps1 | iex          # Windows PowerShell
grok --version
```

从源码构建的依赖：[`rust-toolchain.toml`](rust-toolchain.toml) 锁定的 Rust、[DotSlash](https://dotslash-cli.com)（跑 [`bin/protoc`](bin/protoc)）、以及 `protoc`。详见上游 README 的 Building from source 一节；国内请优先用上面的 Docker 脚本。

根目录 `SOURCE_REV` 记录当前树对应的单仓 commit。

---

## 文档

- 在线文档：[docs.x.ai/build/overview](https://docs.x.ai/build/overview)
- 随仓库的用户指南：[crates/codegen/xai-grok-pager/docs/user-guide/](crates/codegen/xai-grok-pager/docs/user-guide/)（快捷键、slash 命令、配置、MCP、skills、无头模式等）
- 认证：[02-authentication.md](crates/codegen/xai-grok-pager/docs/user-guide/02-authentication.md)
- 自定义模型：[11-custom-models.md](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md)

## 仓库结构

| Path | Contents |
| --- | --- |
| `crates/codegen/xai-grok-pager-bin` | 入口包，编出 `xai-grok-pager` 二进制 |
| `crates/codegen/xai-grok-pager` | TUI |
| `crates/codegen/xai-grok-shell` | Agent 运行时 |
| `crates/codegen/xai-grok-tools` | 工具实现 |
| `crates/codegen/xai-grok-workspace` | 文件系统、VCS、执行、checkpoint |
| `docker/`、`scripts/docker-build-install.sh` | 国内镜像 Docker 编译 |
| `.github/workflows/ci.yml` | 测试 + release 构建 |
| `third_party/` | vendored 依赖（Mermaid 等） |

根目录 `Cargo.toml` 是生成文件，当作只读；改各 crate 自己的 `Cargo.toml`。

## 开发

```sh
cargo check -p <crate>
cargo test -p xai-grok-config
cargo clippy -p <crate>
cargo fmt --all
```

## GitHub Actions

仓库原先没有 CI。本分支在 [`.github/workflows/ci.yml`](.github/workflows/ci.yml) 里跑：

- **test**：`xai-grok-http`、`xai-grok-update`、`xai-grok-shell` 的 `--lib` 测试，以及 `xai-grok-pager-bin` 的 `--bins` 测试（不跑整仓 integration，避免 TTY/网络依赖）
- **build**：`cargo build -p xai-grok-pager-bin --release`，产物以 artifact `grok-linux-x86_64` 上传

触发：`main`、`local-main`、`local/**` 的 push，以及 pull request。GitHub-hosted runner 直连 crates.io / GitHub，不用国内 Docker 镜像。

## 贡献与许可

上游仓库不接受外部 PR，见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。本 fork 的功能分支可以单独整理后视情况向上游提交。

第一方代码为 Apache License 2.0，见 [`LICENSE`](LICENSE)。第三方与 vendored 代码见 [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES)。
