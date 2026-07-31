# crd — AI 平台账户余额查询工具

命令行查询 AI 平台账户余额，支持多平台可扩展架构。

## 功能特性

- 查询 DeepSeek 账户余额：`crd ds`
- 默认简洁文本输出，`--format json` 输出机器可读 JSON
- `-o <文件>` 将输出写入文件
- `Provider` trait 架构，新增平台只需实现接口 + 注册子命令
- API Key 从环境变量读取，不落盘

## 环境要求

- Rust 1.85+（项目使用 edition 2024）
- 操作系统：Windows / Linux / macOS

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
crd ds                            # 文本输出（默认）
crd ds --format json              # JSON 输出
crd ds --format json -o bal.json  # 写入文件
crd ds --help                     # 查看命令帮助
crd --help                        # 查看全局帮助
```

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

1. 在 `src/providers/` 下新建 `xxx.rs`，实现 `Provider` trait：

```rust
pub struct Xxx;

impl Provider for Xxx {
    fn api_key_env_name(&self) -> &'static str { "XXX_BALANCE_API_KEY" }
    fn fetch_balance(&self, api_key: &str) -> Result<BalanceReport, anyhow::Error> { ... }
}
```

2. 在 `src/providers/mod.rs` 中注册模块。
3. 在 `src/main.rs` 中添加对应子命令并分发。

环境变量命名规范：`<平台大写缩写>_BALANCE_API_KEY`（如 DeepSeek 为 `DS_BALANCE_API_KEY`）。
