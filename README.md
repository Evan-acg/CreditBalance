# crd — AI 平台余额与订阅额度查询工具

命令行查询 AI 平台账户余额与 Codex 订阅额度。

## 功能特性

- 查询 DeepSeek 账户余额：`crd ds`
- 查询 OpenRouter 账户余额：`crd or`
- 查询 ChatGPT 订阅下的 Codex 剩余额度与重置时间：`crd gpt`
- 默认简洁文本输出，`--format json` 输出机器可读 JSON
- `-o <文件>` 将输出写入文件
- 默认显示今日消耗（本地时区，基于本地缓存），`-t <单位>` 可变更时间单位
- 每次查询余额自动写入本地缓存
- 余额平台通过 `BalanceProvider` 端口和注册表扩展，Codex 额度通过官方 app-server 查询
- API Key 从环境变量读取，不落盘

## 环境要求

- Rust 1.85+（项目使用 edition 2024）
- 操作系统：Windows / Linux / macOS
- `crd gpt` 需要 Codex CLI，或 Windows 上已登录的 Codex Desktop；查询时不要求 Desktop 界面保持运行

## 架构分层

- `domain/`：余额、Codex 额度和今日消耗规则
- `application/`：查询用例、端口与余额 provider 路由注册表
- `infrastructure/`：平台 HTTP provider、Codex app-server、JSON 历史仓储
- `interfaces/`：CLI 参数解析与输出呈现
- `bootstrap.rs`：组装基础设施实现并注册 provider

## 构建

```bash
cargo build --release
```

产物为 `target/release/crd`（Windows 下为 `crd.exe`，约 1.3MB，已启用 LTO 等体积优化）。

## 部署

将构建产物复制到 PATH 目录即可直接使用，仓库内置一键脚本：

| 平台 | 脚本 | 部署位置 | 参数 |
|---|---|---|---|
| Windows | `build/deploy.ps1` | `C:\Tools\crd.exe` | `-SkipBuild` 跳过构建；`-NoDeploy` 仅构建 |
| Linux/macOS | `build/deploy.sh` | `/usr/local/bin/crd` | `--skip-build` 跳过构建；`--no-deploy` 仅构建 |

```bash
# Windows
./build/deploy.ps1

# Linux/macOS
./build/deploy.sh
```

## 使用说明

### 1. 设置 API Key 环境变量

| 平台 | 环境变量 |
|---|---|
| DeepSeek | `DS_BALANCE_API_KEY` |
| OpenRouter | `OR_BALANCE_API_KEY`（需 management key） |

```powershell
# PowerShell
$env:DS_BALANCE_API_KEY = "sk-..."
```

```bash
# bash
export DS_BALANCE_API_KEY="sk-..."
```

### 2. 查询余额

```bash
crd ds                            # 文本输出（默认，自动显示今日消耗）
crd ds --format json              # JSON 输出（含 today_usage 字段）
crd ds --format json -o bal.json  # 写入文件
crd ds -t d                       # 明确指定今日消耗时间单位（默认即 d=天）
crd or                            # 查询 OpenRouter 余额
crd gpt                           # 查询 Codex 主/次额度窗口及重置时间
crd gpt --format json             # Codex 额度的机器可读 JSON
crd gpt -o codex-quota.txt        # 将 Codex 额度写入文件
crd ds --help                     # 查看命令帮助
crd --help                        # 查看全局帮助
```

### 今日消耗（默认显示）

查询结果默认显示今日（本地时区）消耗的费用，无需额外参数；`-t <单位>` 仅用于变更时间单位（当前支持 `d`=天）。

每次查询的余额都会自动写入本地缓存：

- Windows：`%APPDATA%\crd\cache.json`
- Linux/macOS：`$XDG_CACHE_HOME/crd/cache.json` 或 `~/.cache/crd/cache.json`

今日消耗 = 今天最早一次查询的余额 − 当前余额（当天首次查询显示 `0.00`）。缓存读写失败仅输出警告，不影响查询结果。

JSON 输出格式：`{"balance": {...}, "today_usage": "X.XX"}`。

### Codex 订阅额度

`crd gpt` 复用本机 Codex 的 ChatGPT 登录状态，不读取或输出访问令牌。可通过 Codex CLI 登录：

```bash
codex login
crd gpt
```

命令通过官方 `codex app-server` 的 `account/rateLimits/read` 方法查询主、次额度窗口。文本输出显示本地时间，JSON 中的 `resets_at` 使用 RFC 3339 UTC 时间。剩余额度按 `100 - used_percent` 计算。

Windows 上若未安装 Codex CLI，程序会尝试使用 Codex Desktop 捆绑的 app-server runtime；Desktop 本身可以关闭。其他平台仍需安装 Codex CLI。也可先打开 Codex Desktop 并登录 ChatGPT，再运行 `crd gpt`。

Desktop 捆绑 runtime 的安装路径随版本变化，若未能自动发现，安装 Codex CLI 是最稳定的方式。

`gpt` 支持 `--format text|json` 和 `-o/--output`，不支持余额命令专用的 `--today`。如果提示方法不受支持，请升级 Codex Desktop 或 Codex CLI。

### 退出码

| 退出码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 运行时错误（网络失败、API 返回错误、未设置环境变量等） |
| 2 | 用法错误（未知命令、无效参数等） |

## 输出示例

### 文本格式

```
状态: 可用
总余额 110.00 CNY（充值 100.00 / 赠送 10.00）
```

### JSON 格式

```json
{
  "is_available": true,
  "balance_infos": [
    {
      "currency": "CNY",
      "total_balance": "110.00",
      "granted_balance": "10.00",
      "topped_up_balance": "100.00"
    }
  ]
}
```

## 扩展新平台

1. 在 `src/infrastructure/providers/` 下新建 `xxx.rs`，实现 application 层定义的 `BalanceProvider` 端口：

```rust
pub struct Xxx;

impl BalanceProvider for Xxx {
    fn id(&self) -> ProviderId { ProviderId::new("xxx") }
    fn display_name(&self) -> &'static str { "Xxx" }
    fn fetch_balance(&self) -> Result<BalanceReport> {
        let api_key = read_api_key("XXX_BALANCE_API_KEY")?;
        // 通过 infrastructure/providers/http.rs 调用平台 API
        ...
    }
}
```

2. 在 `src/bootstrap.rs` 中向 `ProviderRegistry` 注册 provider。
3. 注册后，provider ID 会自动作为 CLI 子命令并显示在帮助中；无需修改 CLI 分发器或路由器。

环境变量命名规范：`<平台大写缩写>_BALANCE_API_KEY`（如 DeepSeek 为 `DS_BALANCE_API_KEY`）。
