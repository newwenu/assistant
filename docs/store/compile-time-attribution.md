# 编译耗时归属实测

> 测量日期：2026-10-30
> 被测目标：`src-tauri` workspace（`cargo build --timings`，dev profile，冷启动）
> 采样次数：3 次独立 `cargo clean` + `cargo build --timings`
> 关联文档：`docs/store/vault-optional-build.md`（待编写）

---

## 0. 摘要

| 结论 | 数值 |
|---|---|
| 编译单元总数 | 623（三次一致） |
| 单次串行耗时总和 Σ | 812.3 ~ 1145.9s（中位数 1088.3s） |
| 单次 wall time | 83.1 ~ 117.3s（中位数 96.6s） |
| 平均并行度 | 9.27 ~ 11.86 |
| workspace 自身 13 个 crate 合计 | 71.2s（6.5%） |
| 第三方依赖合计 | 1017.1s（93.5%） |
| 排除 vault + sync + security 可省 | **48 个包 / 78.2s 串行 / 占 7.2%** |
| 预计 wall 收益 | **5 ~ 15s**（占 5% ~ 15%，需落地后实测确认） |

三个必须记住的结论：

1. **耗时集中在少数巨型第三方包**：Top 50 个包合计 517.1s，占总量的 47.5%；仅 `windows` + `windows-sys` + `tauri-utils` 三包就占 143.9s（13.2%）。
2. **vault 的编译收益远低于直觉**：`assistant-vault` 的依赖闭包有 126 个包，但其中 108 个是主功能同样需要的共享依赖。真正能随 vault 一起消失的只有 48 个包。
3. **单次 `--timings` 的单包数字不可直接引用**：同一台机器、同一份代码，`reqwest` 三次测量为 3.2s / 9.6s / 15.9s（5 倍差）。任何精确到 0.1s 的单包结论都必须注明采样次数与中位数口径。

---

## 1. 测量方法与可靠性边界

### 1.1 方法

每次测量都完整执行：

```
cargo clean
cargo build --timings
```

- 机器：同一台开发机，14 核 / 20 逻辑处理器，cargo `jobs=20`、`ncpu=20`。
- 工具链：cargo 1.93.0，dev profile（`unoptimized + debuginfo`）。
- 数据来源：`target/cargo-timings/cargo-timing.html` 内的 `UNIT_DATA` 数组，每个编译单元含 `name` / `start` / `duration`。
- 归口口径：同一包名下的多个编译单元（lib / build-script / proc-macro / bin）耗时相加；跨三次采样取**中位数**。

### 1.2 为什么必须三次采样

三次实测结果：

| 采样 | 编译单元 | Σ 串行耗时 | wall time | 平均并行度 |
|---|---|---|---|---|
| RUN1 | 623 | 1145.9s | 96.6s | 11.86 |
| RUN2 | 623 | 1088.3s | 117.3s | 9.27 |
| RUN3 | 623 | 812.3s | 83.1s | 9.78 |
| **中位数** | **623** | **1088.3s** | **96.6s** | — |

同一份代码，Σ 波动 -25% ~ +5%，wall 波动 -14% ~ +21%。波动来源是机器状态（后台进程、Defender 扫描、CPU 热节流、磁盘缓存），不是代码变化。

单包层面的波动更剧烈（三次实测）：

| 包 | RUN1 | RUN2 | RUN3 | 极差 |
|---|---|---|---|---|
| reqwest | 9.6s | 15.9s | 3.2s | 12.7s（5.0×） |
| image | 6.3s | 16.6s | 9.8s | 10.3s（2.6×） |
| assistant-app | 20.7s | 40.5s | 18.3s | 22.2s（2.2×） |
| zxcvbn | 5.0s | 9.0s | 4.5s | 4.5s（2.0×） |
| tauri-utils | 50.9s | 61.0s | 31.7s | 29.3s（1.9×） |

因此：

- 本文所有单包数字均为**三次中位数**，不适用于精确到秒级的因果推断。
- 包与包之间的**相对量级**（例如 `windows` 远大于 `zxcvbn`）是稳定的，可用于排序与决策。
- 跨文档、跨日期的绝对值**不可比较**。存档文档 compile-time-analysis（已归档）的 171.5s wall 与本次 96.6s 差异全部来自机器状态，与代码无关。

### 1.3 已知缺失项

- **codegen 分段数据缺失**：本次 cargo 1.93.0 的 `--timings` 产物中 `sections` 字段全为 `null`，无法拆分「前端（frontend）」「codegen」「LLVM」耗时。存档文档中的 codegen 数据来自更早的 cargo 版本，本次无法复现，故本文不引用。
- **原始 HTML 产物未入库存档**：RUN1/RUN2 的 timing HTML 仅存在于本机临时目录，未纳入仓库；RUN3 的产物位于 `src-tauri/target/cargo-timings/`（`target/` 属构建产物，不入版本控制）。本文表格即为可追溯的数据记录。

---

## 2. 总览：耗时分布形状

- 623 个编译单元，Σ 1088.3s，wall 96.6s。
- **Top 50 包**合计 **517.1s（47.5%）**。
- **Top 10 包**合计 255.3s（23.5%）。
- 长尾极长：中位耗时 ≥2s 的包有 139 个，其余 400+ 个包均 <2s。

这条分布形状直接决定了优化策略：**优化长尾没有意义，只有动 Top 20 才有可见收益；而 Top 20 里 17 个是主功能必需的基础设施（windows / tauri / tokio / syn / serde / regex 等）。**

---

## 3. 按 workspace crate 归口

13 个 workspace crate 的自身编译耗时（中位数，含其 lib / build-script / bin 单元）：

| crate | 耗时 | 编译单元数 | 说明 |
|---|---|---|---|
| assistant-app | 20.7s | 4 | 主 app（lib + bin + build） |
| assistant-providers | 12.0s | 1 | LLM provider 适配层 |
| assistant-storage | 7.8s | 3 | 含 SQLCipher 相关 |
| assistant-crypto | 6.3s | 1 | api_key + vault_crypto 两模块 |
| assistant-vault | 6.1s | 3 | vault 本体 |
| assistant-config | 3.6s | 1 | |
| assistant-i18n | 3.4s | 3 | |
| assistant-sync | 3.3s | 1 | vault git sync |
| assistant-agent | 2.9s | 1 | |
| assistant-crash | 2.0s | 1 | |
| assistant-security | 1.2s | 1 | |
| assistant-orb | 1.1s | 1 | |
| assistant-tools | 0.8s | 1 | |
| **合计** | **71.2s** | **22** | 占总量 **6.5%** |

要点：**workspace 自身只占 6.5%**。拆分 crate 或排除某个业务 crate，单看自身编译时间收益都很小；真正的成本在它拉进来的第三方依赖。

---

## 4. 按第三方技术栈归口

按包名前缀近似归类（存在交叠，例如 `windows` 同时被 tauri 栈与其它栈引用，归类以主用途为准）：

| 技术栈 | 包数 | 中位耗时合计 | 占总量 |
|---|---|---|---|
| tauri / windows / webview 栈 | 40 | 235.3s | 21.6% |
| http / url / icu 栈 | 19 | 65.7s | 6.0% |
| 图像 / 编解码栈 | 8 | 53.5s | 4.9% |
| 加密 / 口令 / keyring 栈 | 23 | 38.5s | 3.5% |

其中 tauri 栈内部又高度集中于 Windows 绑定：

| 包 | 中位耗时 |
|---|---|
| windows | 54.3s |
| tauri-utils | 50.9s |
| windows-sys | 37.7s |
| webview2-com-sys | 5.2s |
| muda / tray-icon / wry / tao | 6.6 / 2.5 / 2.2 / 2.2s |

**这三包（windows / tauri-utils / windows-sys）合计 142.9s，占总量 13.1%，且它们是任何 Tauri 应用都无法避免的固定成本。**

---

## 5. Top 50 包耗时（三次中位数）

| # | 包 | 中位耗时 | # | 包 | 中位耗时 |
|---|---|---|---|---|---|
| 1 | windows | 54.3s | 26 | png | 6.5s |
| 2 | tauri-utils | 50.9s | 27 | serde_derive | 6.4s |
| 3 | windows-sys | 37.7s | 28 | assistant-crypto | 6.3s |
| 4 | assistant-app | 20.7s | 29 | serde_json | 6.2s |
| 5 | regex-automata | 20.1s | 30 | encoding_rs | 6.2s |
| 6 | syn | 16.8s | 31 | assistant-vault | 6.1s |
| 7 | moxcms | 15.7s | 32 | tauri-plugin-opener | 6.0s |
| 8 | tokio | 15.5s | 33 | futures-util | 5.9s |
| 9 | assistant-providers | 12.0s | 34 | regex-syntax | 5.9s |
| 10 | darling_core | 11.6s | 35 | icu_properties | 5.8s |
| 11 | serde_core | 10.4s | 36 | icu_locale_core | 5.7s |
| 12 | image | 9.8s | 37 | tauri-plugin-dialog | 5.4s |
| 13 | reqwest | 9.6s | 38 | brotli | 5.3s |
| 14 | zerocopy | 9.4s | 39 | webview2-com-sys | 5.2s |
| 15 | tauri | 9.1s | 40 | aho-corasick | 5.1s |
| 16 | pxfm | 9.0s | 41 | keyboard-types | 5.1s |
| 17 | time | 8.8s | 42 | ssh-key | 5.1s |
| 18 | h2 | 8.5s | 43 | zxcvbn | 5.0s |
| 19 | toml | 8.2s | 44 | fancy-regex | 5.0s |
| 20 | assistant-storage | 7.8s | 45 | tiff | 4.8s |
| 21 | serde_with | 7.7s | 46 | urlpattern | 4.6s |
| 22 | minidump-common | 7.6s | 47 | schemars | 4.6s |
| 23 | libsqlite3-sys | 6.9s | 48 | plist | 4.5s |
| 24 | cargo_toml | 6.7s | 49 | hyper-util | 4.5s |
| 25 | muda | 6.6s | 50 | libm | 4.5s |
| | | | | **Top50 合计** | **517.1s** |

值得单独标注的几个"非基础设施"包：

| 包 | 中位耗时 | 引入方 | 排除 vault 后是否消失 |
|---|---|---|---|
| zxcvbn | 5.0s | assistant-vault（密码强度） | 是 |
| ssh-key | 5.1s | assistant-security（ssh_agent） | 是 |
| fancy-regex | 5.0s | assistant-vault | 是 |
| num-bigint-dig | 4.4s | ssh-key → rsa | 是 |
| curve25519-dalek | 4.1s | ssh-key | 是 |
| rsa | 1.8s | ssh-key | 是 |
| arboard | 1.6s | assistant-security（clipboard_guard） | 是 |
| keyring / keyring-core | 0.3 / 1.3s | assistant-vault（主密钥存储） | 是 |
| moxcms / pxfm / image / png / tiff | 15.7 / 9.0 / 9.8 / 6.5 / 4.8s | 截图/图像处理（主功能） | 否 |
| libsqlite3-sys / rusqlite | 6.9 / 3.4s | assistant-storage（主功能）+ assistant-vault | **否**（storage 仍需要） |

---

## 6. vault 可排除收益分析

### 6.1 先修正一个依赖事实

`cargo tree` 实测（`--depth 1` 与 `-i` 反向查询）：

```
assistant-vault 的直接 workspace 依赖 = assistant-crypto, assistant-sync
assistant-security 的 workspace 上游     = assistant-app（直接依赖）
ssh-key 的 workspace 上游               = assistant-security
```

即：

- **`assistant-vault` 不依赖 `assistant-security`**。二者都是被主 app 直接依赖的并列 crate。
- `assistant-security` 与 vault 的耦合发生在**代码层**（`cmd/vault/*` 调用了 `clipboard_guard` / `reveal_guard` / `screen_lock` / `ssh_agent` / `unlock_dialog` / `hibp`），不是 crate 依赖层。
- 因此排除 vault 时必须**同时**排除 security，否则 security 会变成"零消费者但仍在编译"的死重。

### 6.2 三个口径的包数对比

| 口径 | 包数 | 含义 |
|---|---|---|
| `assistant-vault` 依赖闭包 | 126 | 包含 crypto、sync 及全部共享依赖 |
| vault + sync + security 联合闭包 | 246 | 待排除集合 |
| 其中与主功能共享 | 198 | **不能省**（tauri / windows / serde / tokio / storage 的 rusqlite 等） |
| **真正可省（专属依赖）** | **48** | 排除后从依赖图中彻底消失 |

这就是"vault 闭包看起来很大、实际能省的很少"的量化依据：**126 个包里 108 个是主功能也要用的。**

### 6.3 48 个可省包及其耗时

| 包 | 中位耗时 | 包 | 中位耗时 |
|---|---|---|---|
| assistant-vault | 6.1s | ssh-encoding | 0.7s |
| ssh-key | 5.1s | ed25519-dalek | 0.7s |
| zxcvbn | 5.0s | pkcs8 | 0.6s |
| fancy-regex | 5.0s | hex | 0.5s |
| libm | 4.5s | either | 0.5s |
| num-bigint-dig | 4.4s | spki | 0.5s |
| curve25519-dalek | 4.1s | ed25519 | 0.5s |
| ts-rs-macros | 3.9s | bcrypt-pbkdf | 0.5s |
| der | 3.4s | cbc | 0.4s |
| assistant-sync | 3.3s | error-code | 0.4s |
| itertools | 3.1s | base16ct | 0.4s |
| derive_builder_core | 2.3s | spin | 0.4s |
| ts-rs | 2.2s | sec1 | 0.4s |
| ssh-cipher | 2.1s | blowfish | 0.4s |
| rsa | 1.8s | block-padding | 0.4s |
| arboard | 1.6s | const-oid | 0.4s |
| curve25519-dalek-derive | 1.4s | keyring | 0.3s |
| pkcs1 | 1.3s | derive_builder | 0.3s |
| keyring-core | 1.3s | signature | 0.3s |
| windows-native-keyring-store | 1.3s | lazy_static | 0.2s |
| assistant-security | 1.2s | pbkdf2 | 0.2s |
| clipboard-win | 1.1s | num-iter | 0.2s |
| pem-rfc7468 | 1.0s | | |
| derive_builder_macro | 0.9s | | |
| num-integer | 0.8s | | |
| termcolor | 0.8s | | |
| **合计** | **78.2s** | 占总 Σ 1088.3s | **7.2%** |

按来源分组：

| 来源 | 包数 | 耗时 |
|---|---|---|
| vault 自身（assistant-vault + sync + 密码强度 zxcvbn + fancy-regex + ts-rs 栈 + derive_builder 栈 + itertools 等） | 19 | 33.9s |
| ssh 栈（ssh-key / rsa / ed25519 / curve25519 / pkcs / der / cipher 等） | 22 | 30.9s |
| keyring 栈（keyring / keyring-core / windows-native-keyring-store） | 3 | 2.9s |
| 剪贴板栈（arboard / clipboard-win / error-code） | 3 | 3.1s |
| security 自身 | 1 | 1.2s |
| 其它（libm / termcolor / lazy_static / hex / either） | — | 6.2s |

### 6.4 wall time 收益：为什么不能直接说"省 78s"

78.2s 是**串行耗时总和**，实际 wall 收益取决于关键路径。三次采样的模拟结果：

| 采样 | wall（全量） | wall（剔除 48 包后，调度不变） | 差值 |
|---|---|---|---|
| RUN1 | 96.6s | 96.6s | 0s |
| RUN2 | 117.3s | 117.3s | 0s |
| RUN3 | 83.1s | 83.1s | 0s |

差值为 0 的原因：**wall 的终点由主 app 自身决定，而不是由被剔除的包决定**。RUN3 的最后 10 个完成单元：

```
assistant-app  bin        start=80.1s  dur=3.0s   end=83.1s   ← wall 终点
assistant-app  lib        start=66.4s  dur=13.7s  end=80.1s
tauri                     start=61.4s  dur=5.8s   end=67.2s
tauri-plugin-dialog       start=66.0s  dur=0.5s   end=66.4s
...
```

被剔除的 48 个包最晚在 60.3s（RUN3）/ 72.4s（RUN1）/ 73.9s（RUN2）完成，早于 `assistant-app` 的 66.4s 起始。所以移除它们**不会改变终点**，只能通过释放并发槽让后续单元略微提前。

因此 wall 收益只能给出区间估计：

- **下界 0s**：若 48 个包恰好不在关键路径上。
- **上界 ~16s**：若 `assistant-vault`（6.1s）→ `assistant-app` 这条链完全消失，且 `assistant-app` 的起始被同等地提前（RUN3 中 vault 于 60.3s 完成、app 于 66.4s 开始）。
- **合理估计 5 ~ 15s**，占 96.6s 基线的 **5% ~ 15%**。

这个数字必须落地 feature 后实测确认，**不应在方案里承诺 30% 以上的编译加速**。

### 6.5 那么排除 vault 的真实收益是什么

编译时间不是主要收益。按重要性排序：

| 收益 | 量级 | 说明 |
|---|---|---|
| 依赖面缩减 | **48 个包消失** | 供应链攻击面、许可证审计、`cargo deny` 名单、SBOM 条目同步减少 |
| 首次构建串行耗时 | 78.2s | 对 CI 冷缓存构建有实际意义（CI 通常无缓存，wall 收益会接近 Σ 收益） |
| 产物体积 | 需实测 | zxcvbn 内置英文密码词典（约 MB 级）、ssh/rsa/curve25519 代码段、keyring 绑定 |
| 安全审计范围 | 显著 | 排除后不需要审计密码派生、SSH agent、主密钥存储、剪贴板守卫等敏感代码路径 |
| 产品形态 | — | 可发行"无 vault 精简版"，避免用户误以为产品含密码管理功能 |

**注意 CI 与本地开发的差异**：本地增量构建中第三方包已在缓存，78.2s 的收益基本拿不到；CI 冷构建中这 48 个包必须从零编译，收益接近满额。这是评估该方案时最容易被忽略的一点。

---

## 7. 第二层优化：`assistant-crypto` 内部 feature 切分

即使排除 vault，`assistant-crypto` 仍会保留（主功能用 `api_key` 加密 API Key）。但它的 `vault_crypto` 模块会拉入一批主功能根本不需要的加密依赖。

`vault_crypto` 专用、`api_key` 不用的依赖（中位耗时）：

| 包 | 中位耗时 | 用途 |
|---|---|---|
| argon2 | 2.1s | 密码派生（vault 主密钥） |
| data-encoding | 2.4s | bech32 / 编码 |
| hkdf | 1.0s | 密钥派生 |
| poly1305 | 1.0s | AEAD 组件 |
| sha1 | 0.5s | HKDF 旧路径 / SSH |
| cipher | 0.5s | 分组密码抽象 |
| subtle | 0.4s | 常量时间比较 |
| hmac | 0.4s | |
| inout | 0.4s | |
| chacha20poly1305 | 0.3s | vault 数据加密 |
| aead | 0.3s | |
| zeroize | 0.3s | 内存擦除 |
| universal-hash | 0.3s | |
| opaque-debug | 0.2s | |
| cpufeatures | 0.2s | |

合计约 **10.3s**（部分包可能与其它路径共享，实际收益 8 ~ 10s）。

`api_key` 实际需要且必须保留的：`aes` 1.4s、`aes-gcm` 0.4s、`base64` 2.2s、`sha2` 1.8s、`rand` 1.4s、`getrandom` 4.4s、`machine-uid` 0.5s、`generic-array` 2.7s、`digest` 1.0s。

**这一层是纯 crate 内 `[features]` 切分，不涉及跨 crate 边界，风险低、可独立验证。** 但它只对"排除 vault"这一场景有价值——保留 vault 时这些依赖本来就需要。

---

## 8. 与存档文档的差异（为什么旧数字不能直接用）

存档文档 compile-time-analysis（已归档）记录的 617 units / Σ1311.0s / wall 171.5s，与本次 623 units / Σ1088.3s / wall 96.6s 相比：

| 维度 | 差异 | 原因 |
|---|---|---|
| 编译单元 617 → 623 | +6 | 代码新增（crypto / vault / crash 拆分后的新 crate、I23 DTO 等） |
| Σ 1311.0 → 1088.3s | -17% | 机器状态（不是代码） |
| wall 171.5 → 96.6s | -44% | 机器状态（并行度 7.6 → 11.9） |

**判定依据**：`windows`（54.3s vs 113.0s）、`tokio`（15.5s vs 34.6s）、`image`（9.8s vs 24.6s）这些源码与 feature 均未变化的第三方包，耗时下降 40%~60%。代码不可能造成这种变化。

同时，存档文档里"vault 相关闭包占 34.8%"这一结论也不能沿用：它是"vault 依赖闭包总耗时"，未剔除 108 个共享包。按可省口径，真实值只有 **7.2%**。

---

## 9. 对 `vault-optional-build` 方案的启示

1. **不要把编译加速作为方案的主要卖点**。合理预期是 wall 5% ~ 15%（5 ~ 15s），且仅在冷构建时接近上界。若以"编译加速"立项，验收标准应为"CI 冷构建耗时下降 ≥60s"这类可测指标，而非模糊的"更快"。
2. **主要卖点应是依赖面与安全审计范围**：48 个包、22 个 SSH 相关包、3 个 keyring 包、剪贴板栈全部消失。
3. **排除范围必须是 `vault + sync + security` 三件套**，不能只排 vault。security 在 L1 解耦后已无主功能消费者，单独留下只会白编译。
4. **`assistant-crypto` 必须保留**，但建议做 crate 内 `vault-crypto` feature 切分，额外省约 8 ~ 10s。
5. **`rusqlite` / `libsqlite3-sys` 省不掉**：`assistant-storage` 同样依赖 SQLCipher，这部分 10.3s 是主功能固定成本。
6. **`ts-rs` / `ts-rs-macros`（6.1s）不在可省清单内**：ts-rs 已提升为 workspace 依赖，同时服务 `assistant-protocol`（app 侧 165 个 wire DTO → `src/types/protocol.contract.ts`）与 `assistant-vault`（vault 自有 `dto/` → `src/vault/core/contract/vault.contract.ts`），排除 vault 三件套后仍被 protocol 需要。旧表述「仅服务于 vault DTO」已过期（I62）。
7. **验收必须实测 wall**：不要用本文的模拟值（剔除后 wall 差值恒为 0）作为收益承诺，它只证明了"终点不变"，不构成收益上限证明。

### 契约生成实测 wall（`contract:gen` 三段口径）

`npm run contract:gen` 已拆为三段独立任务，每段只编译并运行自己的 crate，互不牵连：

| 段 | 命令（均前置 `cd src-tauri &&`） | 生成物 | 实测 wall（热 target） |
|---|---|---|---|
| vault | `cargo test -p assistant-vault --lib export_bindings` | `src/vault/core/contract/vault.contract.ts` | ~0.8s |
| protocol | `cargo test -p assistant-protocol --test protocol_bindings` | `src/types/protocol.contract.ts` | ~0.6 ~ 0.9s |
| shortcuts | `cargo test -p assistant-config --test shortcut_defaults_bindings` | `src/types/shortcutDefaults.generated.ts` | ~0.55s |
| 合计 | `npm run contract:gen`（三段串行） | 三个产物 | ~2.1s |

拆分收益：改 DTO 只重编 `assistant-protocol`（叶子 crate，依赖闭包仅 serde / serde_json / ts-rs），不再连带 app 全量重编；三段均可从仓库根直接执行。`npm run contract:verify` 在此基础上追加 `git diff --exit-code`，保证生成物字节稳定。

---

## 10. 复现命令

```powershell
# 单次采样（需重复三次取中位数）
cargo clean
cargo build --timings

# 从产物提取单元数据（UNIT_DATA 为 HTML 内嵌 JS 数组）
$t = Get-Content 'target\cargo-timings\cargo-timing.html' -Raw
$d = [regex]::Match($t, 'const UNIT_DATA = (\[.*?\]);', 'Singleline').Groups[1].Value | ConvertFrom-Json

# 依赖闭包
cargo tree -p assistant-vault --prefix none --no-dedupe
cargo tree -i assistant-security --prefix none
```