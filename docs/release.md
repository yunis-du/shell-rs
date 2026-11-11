# 发布

[返回 README](../README.zh-CN.md)

## 本地 macOS DMG

在 macOS 上运行一键打包脚本：

```sh
scripts/build-dmg.sh
```

脚本自动定位仓库、读取 `Cargo.toml` 的版本号，并按本机 Rust 工具链的架构构建 Release 二进制，调用 `packaging/macos/bundle.sh` 生成 App、做本地临时签名，再打包并校验 DMG。镜像里带「应用程序」快捷方式，产物为 `target/dmg/ShellRS-<版本>-macos-<架构>.dmg`，完成后在 Finder 中打开该目录。首次 Release 构建较慢；重复运行会覆盖同名 DMG。

不需要 Apple 开发者证书；这个本地包不启用自动更新，也不会发布到 GitHub。App 的构建编号默认是 `1`，可用 `SHELLRS_BUILD_NUMBER=2 scripts/build-dmg.sh` 指定。

## 首次配置 GitHub

在 [yunis-du/shell-rs](https://github.com/yunis-du/shell-rs) 启用 GitHub Actions。应用直接读取公开的 Releases，不需要登录 GitHub；因此仓库需要公开才能为用户提供应用内更新。

在仓库目录之外用 minisign 生成这个分支自己的密钥：

```sh
minisign -G -p shellrs-update.pub -s shellrs-update.key
```

在仓库的 **Settings → Secrets and variables → Actions** 中配置：

| 类型 | 名称 | 值 |
|---|---|---|
| Variable | `SHELLRS_UPDATE_PUBLIC_KEY` | `shellrs-update.pub` 的第二行 |
| Secret | `MINISIGN_SECRET_KEY` | `shellrs-update.key` 的完整内容 |
| Secret | `MINISIGN_PASSWORD` | 生成私钥时设置的口令；无口令的密钥可省略 |

私钥和口令不要提交到 Git。离线备份私钥：已发布的版本只信任构建时嵌入的公钥，直接换密钥会让它们收不到更新。工作流使用 GitHub 自动提供的 `GITHUB_TOKEN` 发布，不需要另建个人访问令牌。

macOS 默认使用临时签名，生成可下载的 DMG 和更新 ZIP，但没有 Apple 公证，首次打开可能需要在系统设置的「隐私与安全性」中手动允许。要用 Developer ID 签名并公证，在同一仓库配置全部七项 Secret：`APPLE_CERTIFICATE_P12`（base64）、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_TEAM_ID`、`APPLE_API_KEY_P8`（base64）、`APPLE_API_KEY_ID`、`APPLE_API_ISSUER`。正式签名后应保持 bundle ID 和 Team ID 不变；已安装的签名版会拒绝其他团队签名的更新。

## 推送 tag 发布

更新根目录 `Cargo.toml` 的版本号及 `Cargo.lock`，并在 `CHANGELOG.md` 和 `CHANGELOG.zh.md` 各写一节 `## [x.y.z]`。提交这些改动后，推送分支和对应 tag，例如：

```sh
git push origin main
git tag -a v0.1.19 -m "Release v0.1.19"
git push origin v0.1.19
```

[`release` 工作流](../.github/workflows/release.yml) 校验 tag 和版本号，打包 macOS Apple Silicon / Intel 的 DMG 和更新 ZIP、Windows x64 安装版和便携 ZIP，以及 Linux x86_64 AppImage。所有构建成功后，工作流签名并校验产物，生成 `SHA256SUMS` 和签名更新清单，将它们一起发布到 [GitHub Releases](https://github.com/yunis-du/shell-rs/releases)。在 Actions 手动运行工作流只打包，可从运行详情下载产物。

正式版包含 `stable.json` 和 `beta.json`；像 `v0.1.19-beta.1` 这样的预发布版标记为 Pre-release，只包含 `beta.json`。客户端按语义版本选出所选通道的最新版，验签后从同一个 Release 下载当前平台的包，再校验大小和 SHA-256。清单结构见 [示例](../packaging/manifest.example.json)。

此前从上游或本地脚本安装的版本不会自动迁移到 fork 的更新源。先从此仓库的 Releases 手动安装一次，以后由该分支更新。
