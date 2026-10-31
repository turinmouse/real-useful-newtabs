<div align="center">

# real-useful-newtabs

**让新标签页成为你的工作空间。**

在 Chrome 中，用平铺画布整理书签与标签组。

[English](README.md) · [简体中文](README.zh-CN.md)

[开始使用](#开始使用) · [功能](#功能) · [开发](#开发) · [反馈问题](https://github.com/turinmouse/real-useful-newtabs/issues)

[![License](https://img.shields.io/badge/License-GPL--3.0--only-blue)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-2024_edition-orange?logo=rust)
![UI](https://img.shields.io/badge/UI-egui-blue)
![Platform](https://img.shields.io/badge/Chrome-Manifest_V3-green?logo=googlechrome&logoColor=white)

</div>

## 为什么做这个项目？

新标签页也可以用来整理正在进行的工作。将书签文件夹与保存的标签组并排摆放，为不同场景建立独立画布，下次打开时继续使用保存的布局。

项目使用 Rust、egui/eframe 和 WebAssembly 构建。Chrome 扩展是主要使用方式，同时提供用于界面开发的原生应用。

## 功能

| 功能 | 可以做什么 |
| --- | --- |
| 多画布 | 创建、重命名和切换工作空间，每个画布拥有独立布局。 |
| 平铺窗口 | 拖动标题调整位置，拖动分隔线调整大小；超出区域的内容在窗口内滚动。 |
| 书签管理 | 按标题和网址搜索，将文件夹单独打开为窗口，重命名、删除及拖动排序。 |
| 保存的标签组 | 浏览分组、搜索组内网址，将记录的页面重新打开到对应的 Chrome 标签组。 |
| 标签组增强 | 按需将页面或域名关联到已命名的标签组，并管理自定义网址规则。 |
| 新标签页行为 | 按需将新标签页保持在最右侧，或复用同一窗口中已有的新标签页。 |
| 外观 | 切换深色与浅色主题。 |

书签拖动排序仅在未进行书签筛选时可用。标签组增强、新标签页靠右和新标签页复用默认关闭。

## 开始使用

### 1. 准备工具

安装 Git 和 [mise](https://mise.jdx.dev/)。项目要求 Rust **1.95 或更新版本**；Rust、WebAssembly 目标、Node.js 和 wasm-bindgen CLI 均由 mise 管理。

```sh
git clone https://github.com/turinmouse/real-useful-newtabs.git
cd real-useful-newtabs
mise trust
mise install
```

`mise.toml` 中的 CLI 版本必须与 `Cargo.lock` 中的 `wasm-bindgen` 一致，当前为 `0.2.129`。依赖版本变化时请同步更新。

### 2. 构建扩展

```sh
mise run release
```

构建后的未打包扩展位于 `dist/`。

### 3. 在 Chrome 中加载

1. 打开 `chrome://extensions`。
2. 开启**开发者模式**。
3. 点击**加载已解压的扩展程序**，选择本项目的 `dist/` 目录。
4. 打开一个新标签页。

重新构建后，在扩展卡片上点击**重新加载**，再打开新的标签页。如果移动或重命名了项目目录，请检查 Chrome 加载的是当前的 `dist/` 路径。

## 按自己的习惯使用

- 从右侧导航打开 **Bookmarks（书签）**、**Tab Groups（标签组）** 或 **Settings（设置）**。
- 将窗口标题拖到另一窗口的边缘以分割布局，拖到中央以交换位置。
- 为工作、个人浏览等不同场景创建独立画布。
- 在 **Settings** 中选择新开标签页是否切换到前台，并开启需要的新标签页行为。
- 使用标签组增强时，请先为 Chrome 标签组命名。在支持标签右键菜单的 Chrome 中，通过 **Tab group enhancement → Pin domain / Pin full path** 记录关联；也可以在标签组工具中管理规则。

全局设置会在已打开的新标签页之间同步。每个页面的当前画布与布局保持独立，同时保存以便后续使用。

## 权限与数据

| Chrome 权限 | 用途 |
| --- | --- |
| `bookmarks` | 根据你的操作读取、重命名、删除和移动书签。 |
| `contextMenus` | 在浏览器支持时提供标签组关联菜单。 |
| `favicon` | 在条目旁显示网站图标。 |
| `tabs` | 打开与管理标签页，实现导航及可选的新标签页行为。 |
| `tabGroups` | 读取与管理 Chrome 标签组。 |
| `storage` | 保存设置、布局、标签组及会话绑定。 |

设置、布局与保存的标签组存储在浏览器中，临时分组绑定使用会话存储。页面之间的实时设置更新不等于跨设备同步。修改书签会影响你的实际 Chrome 书签。

## 开发

| 命令 | 用途 |
| --- | --- |
| `mise run dev` | 运行原生界面进行开发；Chrome API 功能需要在扩展中使用。 |
| `mise run build-extension` | 将调试版扩展构建到 `dist/`。 |
| `mise run release` | 将优化后的扩展构建到 `dist/`。 |
| `mise run check` | 检查原生目标和 WebAssembly 目标。 |
| `mise run test` | 运行 Rust 与 JavaScript 测试。 |
| `mise run fmt` / `mise run fmt-check` | 格式化 Rust 代码 / 检查格式。 |
| `mise run clippy` | 运行 Rust 静态检查。 |

```text
src/          Rust 应用、画布布局、状态及工具
extension/    扩展清单、HTML、JavaScript 桥接与后台服务
assets/       内置资源
mise.toml     开发与构建任务
DESIGN.md     界面行为与实现约定
```

`dist/` 和 `target/` 为生成目录。请修改源文件后重新构建。

## Tag 发布

推送到 `master` 时会运行检查、测试和优化编译，建立 mise 工具缓存及 Cargo 依赖和编译缓存。Tag 构建可以复用默认分支的缓存；仅 tag 会打包并发布 Release 附件。

推送任意 tag（例如 `git push origin v0.1.0`）后，GitHub Actions 会通过 mise 执行检查、测试和优化编译，并创建 GitHub Release，附带 `real-useful-newtabs-<tag>.crx` 和 `.tgz`。重新运行工作流会替换已有 Release 中的同名附件。`.tgz` 内是未打包扩展，解压后可通过 `chrome://extensions` 加载。

如需保持不同版本的 CRX 扩展 ID 一致，请将 PEM 格式的 RSA 私钥配置为仓库 Actions Secret `CRX_PRIVATE_KEY`。未配置时，每次运行使用临时密钥，扩展 ID 会变化。签名私钥不会包含在发布附件中。扩展版本号取自 `extension/manifest.json`，发布新版本 tag 前请先更新该文件。

## 参与贡献

欢迎提交问题、修复与文档翻译。[反馈问题](https://github.com/turinmouse/real-useful-newtabs/issues)时，请附上复现步骤和浏览器版本。

修改界面前请阅读 [DESIGN.md](DESIGN.md)。提交代码变更前运行 `mise run check` 和 `mise run test`，并在 Chrome 中验证界面，包含窄窗口布局。

新增 README 翻译时，请创建 `README.<语言标签>.md`，保持相同的章节结构，并在各语言版本中添加切换链接。功能或安装步骤变化时，请同步更新已有版本。这里的多语言支持指文档，不代表应用界面已经本地化。

## 许可证

本项目采用 **GNU 通用公共许可证第 3 版（仅此版本）**（`GPL-3.0-only`）。完整条款见 [LICENSE](LICENSE)。

## 致谢

项目基于 [egui/eframe](https://github.com/emilk/egui) 和 [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) 构建。README 的简洁简介与导航结构参考了 [Zen Browser](https://github.com/zen-browser/desktop)。
