# 测试组合矩阵

这份文档定义验收场景要覆盖哪些维度，以及每个维度怎么断言。`scenarios/*.json` 按它编写，
场景的逐条索引和实际数量见 `scenarios/README.md`。矩阵变动时，先改这里，再改场景。

## 1. 维度

一次运行 = 场景 × 网络预设 × 响应风格 × 客户端 adapter：

- **场景**：见 §3 各个领域。
- **网络预设**：`normal`、`weak`、`lossy`、`offline`、`blackhole`、`flap`、`dns`，定义见 CONTRACT §6。
- **响应风格**：`http` 表示失败时返回标准 HTTP 状态码；`business` 表示 HTTP 一律 200，状态码放在 code 里。定义见 CONTRACT §2.1。
  默认每个场景两种风格都跑。预期优先写成与风格无关的 `status:<n>`；只有两种风格下行为确实不同时，才用 `<网络>@<风格>` 分开写。
  这个维度专门验证客户端在 business 风格下，能不能从 HTTP 200 的响应里识别出 401 并触发刷新，识别出 403 并触发越权处理，识别出 500 并触发重试。
- **adapter**：每个客户端自己声明。axp 是 `http`（Node http 模块）和 `fetch`（undici）；dioman 是 `io`（dart:io HttpClient），以后可以再加。

不适用的组合要在场景的 `expect.<网络>.skip` 里写明原因，不能空着。

## 2. 网络维度的统一断言规则

| 网络 | 断言什么 | 不断言什么 |
|---|---|---|
| `normal` | 结果、服务端命中数、客户端事件序列、完成顺序，全部精确断言 | – |
| `weak` / `lossy` | 最终结果、资源释放（`pending_after = 0`、loading 成对出现） | 耗时、完成顺序（barrier 场景例外，顺序由服务器保证） |
| `offline` / `dns` | 错误为 `network`；服务端 `total = 0`；loading 成对；`pending_after = 0` | – |
| `blackhole` | 错误为 `timeout`；`pending_after = 0` | – |
| `flap` | 带 retry 的请求最终成功；无重试的对照组设置短于离线窗口的连接预算，首发允许 `network` / `timeout`，恢复后成功 | 重试的具体次数；未设短连接预算时不能断言操作系统必在恢复前返回错误 |

主动取消与离线连接拒绝并发时，按最先完成的错误返回：可以是 `network` 或 `canceled`。
特别是 Windows 的连接拒绝完成可能晚于取消定时器；仍须断言服务端 total=0 和 pending_after=0。
flap 等抖动网络只要求 loading 成对且最终关闭，不固定回调次数；缓存命中可能不再显示 loading。

dioman 的完整报告枚举所有场景 × 七网络 × 两风格（io）；场景未声明的网络/风格明确跳过。
本轮用户要求遇 429/503 停止，因此主动注入这两个状态的场景明确跳过，不计通过。
17 插件组合采用可复现的 154 项确定性策略：17 单开、136 精确双开和全部同开。
每项经过真实 netem 与服务器；这覆盖所有插件对，不代表执行了全部 131071 个子集。

## 3. 场景领域

★ 表示 2026-10-27 应用户要求补充的令牌类场景。

### 3.1 鉴权：坏令牌 ★

| 令牌状态 | refresh 可用 | refresh 坏或缺失 |
|---|---|---|
| 无令牌（无头 / 空 Bearer） | 不刷新，判 `unauthenticated`，`refresh = 0` | 同左 |
| 假令牌：garbage、bad_signature、tampered_payload、wrong_scheme | 刷新 1 次后重放成功 | 最多刷新 1 次，判 `expired`，**不能循环刷新** |
| 过期令牌 | 刷新后重放成功 | 判 `expired` |
| 旧令牌：logout 后、revoke 后、strict_rotation 下 refresh 后被顶替 | 刷新后重放成功 | 判 `expired` |
| 旧 refresh（previous） | – | 服务端判重用（reason=reused），整族吊销，判 `expired` |
| `no_refresh_` 用户的令牌过期 | – | 直接判 `expired`，不调用刷新接口 |

并发版本：10 个请求带着同一个假令牌或过期令牌同时发出，只刷新 1 次（`by_path /t/auth/refresh = 1`）。
默认轮换（旧 access 在过期前仍然有效）和 strict_rotation 两种后端都要覆盖。

### 3.2 鉴权：流程

- 登录 → 访问 → 登出 → 再访问失败。
- 刷新失败：所有请求判 `expired`，`expired` 回调只触发 1 次。
- 慢刷新（refresh 带 `delay_ms`）期间：新发起的请求排队，拿到新令牌后再发；在这期间取消请求。
- business 风格（HTTP 200 + code 401）也要触发刷新，由风格维度自动覆盖；另外单独写一条：只返回 code=401、没有 reason 的后端也能走通刷新流程。
- 403 越权：触发 `denied`，不刷新。
- 匿名接口和受保护接口混合并发。

### 3.3 重试

- 前 N 次 500 后恢复；重试耗尽。
- 4xx 不重试；POST 默认不重试。
- 遵守 `Retry-After`。
- `fail_every` 周期性故障。
- 业务码失败是否重试，按各库的配置决定。
- 重试中遇到 401：先刷新，再继续。
- 每次重试的请求体都要和首次一致，用 echo 回显出来比对。

### 3.4 并发

- barrier 放行顺序 `asc`、`desc`、`shuffle`、`given`，交叉组合全部成功、全部失败、部分失败。
- 大并发 n=32 和 n=64：检查 `peak_inflight` 和完成顺序。

### 3.5 缓存

- 命中缓存时不到达服务器。
- TTL 过期后重新请求。
- 业务失败的响应不入缓存。
- 参数（query/body）不同的请求不串缓存。
- `clear_cache` 生效。
- 切换用户后不串缓存（scope）。

### 3.6 共享去重（share）

- 同 key 并发只发 1 次。
- `start`、`end`、`race` 三种策略。
- 首发请求失败时，每个跟随者各自拿到错误。
- 跟随者单独取消，不影响首发请求。

### 3.7 取消

- 取消单个请求：服务端记录 `aborted ≥ 1`。
- `cancel_all`。
- 取消与重试、刷新交错发生。
- 结束后 `pending_after = 0`。

### 3.8 loading

- 单个请求：显示、隐藏各一次。
- 多个请求并发：也只有一次显示、一次隐藏。
- 重试期间不闪烁，序列为 `[true,false]`。
- 请求失败也会关闭。
- 在 `delay` 时间内完成的请求不显示 loading。

### 3.9 notify / normalize

- 一次失败只通知一次；`silent` 请求不通知。
- 业务失败归类为 `business:<code>`。
- 成功信封要解包；没有信封的响应原样透传。

### 3.10 坏响应

- `fault` 的 8 种取值各一条：hang、close、half_body、slow_body、bad_json、empty、html、no_envelope。
- 大响应：`size`。

### 3.11 网络专项

- 各核心场景在 7 种网络预设下的表现。
- 重点：flap + retry 能恢复；offline 立即失败且资源释放干净；blackhole 依靠超时退出。

### 3.12 timeout / breaker / offline

这一组只有 dioman 有对应插件，axp 缺这些能力会自动跳过。

- 超时：连接超时、接收超时、总超时。
- 熔断：连续失败后打开，打开期间快速失败（`breaker_open`），`open_ms` 后半开探测，成功则恢复。
- 离线：`/t/ping` 探测判定无网后，直接返回 `offline` 错误；网络恢复后放行。

### 3.13 插件组合

- cache + share + retry。
- auth + retry + loading + notify：一次逻辑请求只有一对 loading、最多 1 次通知、1 次刷新。
- cancel + share。
- repath + key + cache。

## 4. 通用断言

每次运行结束后，无论场景是否声明，执行器都要检查：

- `pending_after = 0`：客户端没有未结束的请求。
- loading 回调成对出现：true 和 false 的数量相等，并且最后一个是 false。
- 服务端 `inflight = 0`：网络断开时由 netem 关闭连接后再检查。

## 5. dioman 插件五方法复核（2026-10-27）

新增 `scenarios/plugins.json` 由 dioman 的 `accept/build_plugin_matrix.dart` 生成。
单体、全量、边界和配对场景均复用已有 schema；每场景两种风格，经独立 netem 代理执行。
配对使用 32×17 二元正交数组，独立单测检查全部 136 对的四种开关状态；全关是合法基线。
全量业务流在 normal、weak、flap 下检查登录、访问、刷新、重试、缓存、共享、取消和通知。
规则依据实际插件 ordering 与 extra 注册表执行，证据中的 rules 字段逐条登记。
公开 API 参数覆盖、边界取值覆盖和源码行覆盖独立报告；少量典型值不代表所有公开选项已覆盖。
历史“未声明组合”不作为新的合法不适用理由；尚未补齐的组合必须在复核报告列为未完成。
主动注入 429/503 的用例在本轮停机约束下仍不可真实执行；可用 500 验证一般重试路径，
但这不替代 429/503 状态选择与 Retry-After 的专门验收证据。


### 5.1 本轮最终已核实范围（整项未完成）

79个方法场景：单体34格、全量6格、部分边界56格、配对64格，共160格真实通过。
规则断言记录于同批结果的rules字段，其中全开6格含全部17插件；不重复计为额外请求。
另有安全状态子场景8格及share重放准备异常回归2格真实通过。
429/503的5个专门场景现在对7网络×2风格共70格显式skip，其中56格属于历史缺声明项；
跳过理由来自本轮用户的遇429/503即停约束，不计通过。其余历史缺声明3477格尚未执行。
单体基础路径通过不代表全部公开选项，部分边界通过不代表全部边界；每插件待补项见中文矩阵。
旧136精确双开集合在17开关中可覆盖每对四种状态，但旧证据仅http，不能替代本轮双风格配对。
