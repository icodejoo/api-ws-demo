<!-- 由 tools/gen-index.mjs 生成，勿手改。改完 scenarios/*.json 后运行：node tools/gen-index.mjs -->
# 场景目录

> 本文件由 `tools/gen-index.mjs` 生成，**勿手改**。`node tools/validate.mjs --check-index` 会检查它是否过期。

与语言无关的客户端验收场景，格式见 `../scenario.schema.json`，契约见 `../CONTRACT.md`。axp（TS/axios）与 dioman（Dart/dio）共用同一份。
校验：`node tools/validate.mjs`（合法 JSON、schema 关键规则、id 唯一、expect 覆盖 networks 与 styles、results 引用的 id 存在）。

共 **443** 个场景，19 个文件，每个文件顶层是场景数组；按（网络 × 风格）展开共 **1346** 次运行。

## 约定（执行器必读）

- **每个场景 × 每个声明的网络 × 每种风格**各跑一遍（风格默认 http、business 都跑，见 CONTRACT §2.1；`styles` 可收窄）。
  预期里的错误优先用 `status:<n>`（与风格无关）。需要区分风格时才用 `<网络>@business` 键。
- **business 风格要求客户端把"HTTP 200 + code=n"也识别成逻辑状态 n**：涉及 status/fail_times/fail_every/barrier fail 的场景已自动配上 `normalize` 插件并在 `requires` 里声明。
  retry / breaker / notify 等对失败的判定，要求按**逻辑状态**（两种风格一致）生效，这是本目录有意为之的要求。
- **expect 没用 '*'**：每个网络都显式写出，避免 results/server/client 浅合并时 weak 与 offline 的 server 断言互相污染。weak/lossy/flap 不含 `order`/`duration_ms`。
- **weak/lossy/flap 只断言最终结果与资源释放**；offline/dns/blackhole 下无鉴权的场景 `server.total eq 0`。
- **login/logout/revoke/server_reset 走旁路**（CONTRACT 附录 B.2）：不经过 netem 和插件链，所以含 login 的场景也可以跑 offline/dns/blackhole，被测请求应得到 network/timeout，**不能误判 expired，也不能清掉 token**；这类场景用 `by_path`（旁路登录会计入服务端 total）。
- **revoke 默认 scope=all**（access+refresh 都吊销，应判 expired）；要测"刷新后恢复"必须显式写 `scope:"access"`。
- **`style` 步骤**在运行中切换响应风格，用到它的场景 `styles` 只写 `http`。
- **`/t/mock/*`**：mock 插件的基址固定为 `<baseURL>/t/mock`，场景用 `data.mock=true` 与真实路径 by_path 为 0 判断路由。
- **status 与 code 同时给出**（附录 B.1）：http 风格归 `status:<n>`，business 风格归 `business:<code>`，场景用 `@business` 键区分（`faults.status-with-business`、`normalize.http-error-wins`）。
- **blackhole** 场景带 `client.base_timeout_ms=4000`；weak/blackhole/flap/lossy 场景 `timeout_ms=40000`。
- **server 计数**：`by_path` 的 key 是请求路径，缺失按 0 计；登录、刷新也计入 `by_path`（所以 auth 场景用 by_path 而不用 total）。
- **`from_cache: true`** = 服务端日志里没有该请求。
- **repeat 的内部 id** 为 `<id>#<i>`，i 从 0 开始。
- **`data` 子集匹配**：客户端最终拿到的是信封里的 `data`（CONTRACT 附录 A.1，`data.request.body.json` 为解析后的请求体）。
- **并发 32/64 的场景假设执行器没有每主机连接数限制**（浏览器环境下要放宽或跳过）。
- 中立插件选项扩展：`auth.refresh_query`、`key.scope='user'`、`retry.methods`、`cache.methods`/`cache.max`（见 schema pluginOptions）。映射不了必须显式报错。
- **细分能力与新字段**（CONTRACT §7）：`multipart`（`body.multipart`，文件内容按 seed 的 LCG 算法生成）、`keep-alive`、`share-retry`、`timeout-send`（`timeout.send_ms`）、`mock-in-chain`；缺这些能力的执行器按"缺能力"跳过。结果项 `same_as` 要求两个成功请求 data 的同一字段相等（如 `meta.conn`）。

## 覆盖矩阵（场景数：领域 × 网络预设）

| 领域 | 合计 | normal | weak | lossy | offline | blackhole | flap | dns |
|---|---|---|---|---|---|---|---|---|
| 基础（basic.json） | 22 | 22 | 14 | 1 | 8 | 5 | 1 | 7 |
| 鉴权（auth.json） | 73 | 72 | 27 | 0 | 10 | 10 | 0 | 9 |
| 重试（retry.json） | 49 | 45 | 10 | 8 | 5 | 4 | 5 | 4 |
| 缓存（cache.json） | 32 | 32 | 4 | 0 | 1 | 1 | 0 | 1 |
| 请求合并（share.json） | 21 | 20 | 4 | 0 | 2 | 2 | 0 | 2 |
| 并发（concurrency.json） | 16 | 16 | 9 | 0 | 1 | 1 | 0 | 1 |
| 取消（cancel.json） | 13 | 12 | 2 | 0 | 1 | 2 | 0 | 1 |
| 加载状态（loading.json） | 21 | 21 | 6 | 1 | 4 | 3 | 1 | 3 |
| 通知与信封（notify-normalize.json） | 22 | 22 | 6 | 1 | 3 | 1 | 0 | 2 |
| 故障响应（faults.json） | 16 | 16 | 12 | 1 | 1 | 1 | 1 | 1 |
| 网络条件（network.json） | 14 | 4 | 2 | 1 | 3 | 3 | 3 | 4 |
| 超时（dioman）（timeout.json） | 20 | 19 | 1 | 0 | 0 | 1 | 0 | 0 |
| 熔断（dioman）（breaker.json） | 19 | 18 | 2 | 0 | 1 | 0 | 0 | 1 |
| 离线（dioman）（offline.json） | 9 | 3 | 1 | 0 | 4 | 2 | 1 | 1 |
| 路径/键/过滤（repath-key-filter.json） | 18 | 18 | 3 | 0 | 0 | 0 | 0 | 0 |
| 组合交互（combos.json） | 33 | 31 | 6 | 4 | 1 | 0 | 1 | 1 |
| 模拟（mock.json） | 8 | 8 | 2 | 0 | 1 | 1 | 0 | 1 |
| filter（filter.json） | 3 | 3 | 3 | 0 | 0 | 0 | 0 | 0 |
| plugins（plugins.json） | 34 | 33 | 1 | 0 | 0 | 0 | 1 | 0 |
| **合计** | 443 | 415 | 115 | 17 | 46 | 37 | 14 | 39 |

（一个场景可声明多个网络，所以每行网络列之和大于合计。）

## 风格统计（CONTRACT §2.1）

| 领域 | 两种风格都跑 | 仅 http | 仅 business | 含 `@business` 专属预期 |
|---|---|---|---|---|
| 基础 | 22 | 0 | 0 | 2 |
| 鉴权 | 70 | 3 | 0 | 1 |
| 重试 | 42 | 7 | 0 | 1 |
| 缓存 | 32 | 0 | 0 | 1 |
| 请求合并 | 21 | 0 | 0 | 1 |
| 并发 | 16 | 0 | 0 | 0 |
| 取消 | 13 | 0 | 0 | 1 |
| 加载状态 | 21 | 0 | 0 | 1 |
| 通知与信封 | 20 | 2 | 0 | 3 |
| 故障响应 | 15 | 1 | 0 | 2 |
| 网络条件 | 14 | 0 | 0 | 0 |
| 超时（dioman） | 20 | 0 | 0 | 1 |
| 熔断（dioman） | 19 | 0 | 0 | 1 |
| 离线（dioman） | 9 | 0 | 0 | 1 |
| 路径/键/过滤 | 18 | 0 | 0 | 3 |
| 组合交互 | 30 | 3 | 0 | 0 |
| 模拟 | 6 | 2 | 0 | 1 |
| filter | 3 | 0 | 0 | 0 |
| plugins | 34 | 0 | 0 | 0 |
| **合计** | 425 | 18 | 0 | 20 |

## 场景索引

方括号内依次是网络预设，竖线后是收窄的风格（缺省表示两种都跑）。

### 基础（basic.json，22 个）

- `basic.get-ok` [normal,weak,offline,dns,blackhole] 最简 GET 成功
- `basic.post-json-body` [normal,weak,offline,dns,blackhole] POST JSON 请求体原样到达
- `basic.unicode-body` [normal,weak,offline] Unicode 与表情请求体
- `basic.query-echo` [normal,weak,offline,dns,blackhole] 查询参数原样到达
- `basic.custom-header` [normal,weak] 自定义请求头到达服务器
- `basic.all-methods` [normal,weak,offline,dns] 五种方法都能走通
- `basic.public-path` [normal,weak] 匿名 /t/public 路径
- `basic.http-404` [normal,weak,offline,dns,blackhole] HTTP 404 归为 status:404
- `basic.status-matrix` [normal,weak,lossy,offline,blackhole,flap,dns] 常见 4xx/5xx 状态码分类
- `basic.business-code-without-normalize` [normal,weak] 未装 normalize 时业务码不当错误
- `basic.sequential-hits` [normal,weak] 顺序请求命中计数递增
- `basic.custom-key-param` [normal] key 参数独立计数
- `basic.server-reset` [normal] server_reset 清空轮次状态
- `basic.delay-respected` [normal,weak] 服务端延迟被如实等待
- `basic.large-sequence` [normal] 连续 20 次请求稳定
- `basic.parallel-independent` [normal,weak,offline,dns] 并行请求各自独立
- `basic.head-request` [normal] HEAD 请求
- `basic.status-201-empty-data` [normal] 2xx 非 200 也算成功
- `basic.safe-status-matrix` [normal,weak] 不含 429/503 的状态码子矩阵
- `plugins.single-envs` [normal] envs 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.single-log` [normal] log 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `basic.keep-alive-recovery-isolated` [normal] 连接复用下，401 恢复失败与 500 重试耗尽之后，同一连接上的下一个响应完整无污染

### 鉴权（auth.json，73 个）

- `auth.anon-and-private-mix` [normal,weak,offline,dns,blackhole] 匿名与鉴权接口混合
- `auth.logout-then-private-fails` [normal,weak] 登出后访问受保护接口失败
- `auth.relogin-other-user` [normal,weak] 换用户登录后不串身份
- `auth.expired-refresh-replay` [normal,weak,offline,dns,blackhole] access 过期 401 后刷新并重放
- `auth.natural-expiry-refresh` [normal] 令牌自然过期后刷新
- `auth.two-rounds-refresh-chain` [normal,weak] 连续两轮过期刷新
- `auth.concurrent-401-single-refresh-8` [normal,weak,offline,dns,blackhole] 8 个并发请求同时 401 只刷新一次
- `auth.concurrent-401-single-refresh-32` [normal] 32 个并发请求同时 401 只刷新一次
- `auth.concurrent-two-waves` [normal] 两波并发各刷新一次
- `auth.refresh-expired-session` [normal] refresh 也过期则判会话过期
- `auth.refresh-expired-concurrent` [normal] 刷新失败时并发请求全部以 expired 失败
- `auth.refresh-fail-times` [normal] 刷新接口 5xx 失败判过期
- `auth.relogin-after-expired` [normal] 过期后重新登录能恢复
- `auth.revoke-kicked` [normal,weak,offline,dns,blackhole] 服务端踢下线后的访问
- `auth.code-only-401-refresh-once-then-expired` [normal,weak] 只有 code=401、不带 reason：刷新一次、失败判过期、不循环
- `auth.code-only-401-recovers` [normal,weak] 只有 code=401：刷新后重放成功
- `auth.code-only-401-no-refresh-token` [normal] 只有 code=401 且拿不到 refresh
- `auth.code-only-401-concurrent` [normal] 并发的无 reason 401 只刷新一次
- `auth.deny-403` [normal,weak,offline,dns,blackhole] 403 越权回调 denied 且不刷新
- `auth.public-401-no-refresh` [normal] 匿名接口返回 401 不触发刷新
- `auth.public-ignores-expired-token` [normal] 本地 token 已过期时访问匿名接口不刷新
- `auth.post-replay-body-intact` [normal,weak,offline,dns,blackhole] 刷新重放后请求体与首发一致
- `auth.requests-during-refresh-queue` [normal,weak] 刷新进行中新发的请求排队等新 token
- `auth.slow-refresh-cancel` [normal] 慢刷新期间取消请求
- `auth.slow-refresh-cancel-all` [normal] 慢刷新期间 cancel_all
- `auth.logout-inflight` [normal] 登出时仍在途的请求
- `auth.offline-refresh-keeps-session` [normal] 断网时刷新失败不算会话过期
- `auth.no-refresh-user-expired` [normal] 拿不到 refresh 令牌的用户过期后判过期
- `auth.token-none-refresh-ok` [normal,weak] 无令牌（none）+ refresh ok
- `auth.token-none-refresh-none` [normal,weak] 无令牌（none）+ refresh none
- `auth.token-none-refresh-bad` [normal,weak] 无令牌（none）+ refresh bad
- `auth.token-empty-refresh-ok` [normal,weak] 无令牌（empty）+ refresh ok
- `auth.token-empty-refresh-none` [normal,weak] 无令牌（empty）+ refresh none
- `auth.token-empty-refresh-bad` [normal,weak] 无令牌（empty）+ refresh bad
- `auth.token-garbage-refresh-ok` [normal,weak,offline,dns,blackhole] 假令牌（garbage）+ 可用 refresh
- `auth.token-garbage-refresh-none` [normal] 假令牌（garbage）+ 无 refresh
- `auth.token-garbage-refresh-bad` [normal] 假令牌（garbage）+ refresh 也是假的
- `auth.token-bad-signature-refresh-ok` [normal,weak,offline,dns,blackhole] 假令牌（bad-signature）+ 可用 refresh
- `auth.token-bad-signature-refresh-none` [normal] 假令牌（bad-signature）+ 无 refresh
- `auth.token-bad-signature-refresh-bad` [normal] 假令牌（bad-signature）+ refresh 也是假的
- `auth.token-tampered-payload-refresh-ok` [normal,weak] 假令牌（tampered-payload）+ 可用 refresh
- `auth.token-tampered-payload-refresh-none` [normal] 假令牌（tampered-payload）+ 无 refresh
- `auth.token-tampered-payload-refresh-bad` [normal] 假令牌（tampered-payload）+ refresh 也是假的
- `auth.token-wrong-scheme-refresh-ok` [normal,weak] 假令牌（wrong-scheme）+ 可用 refresh
- `auth.token-wrong-scheme-refresh-none` [normal] 假令牌（wrong-scheme）+ 无 refresh
- `auth.token-wrong-scheme-refresh-bad` [normal] 假令牌（wrong-scheme）+ refresh 也是假的
- `auth.token-garbage-concurrent-refresh-ok` [normal,weak] 并发请求带同一假令牌只刷新一次
- `auth.token-garbage-concurrent-refresh-bad` [normal] 并发假令牌 + 坏 refresh 全部判过期
- `auth.token-garbage-concurrent-no-refresh` [normal] 并发假令牌 + 无 refresh
- `auth.token-none-concurrent` [normal] 并发无令牌请求都不刷新
- `auth.token-no-refresh-user-garbage` [normal] no_refresh 用户 + 假令牌
- `auth.token-no-refresh-user-none` [normal] no_refresh 用户 + 无令牌
- `auth.token-previous-after-logout` [normal] 登出后使用旧令牌
- `auth.token-previous-after-revoke` [normal] revoke 后旧令牌 + 可用 refresh
- `auth.token-strict-previous-access` [normal] strict_rotation：被顶替的旧 access
- `auth.token-nonstrict-previous-access` [normal] 默认模式：刷新后旧 access 仍有效
- `auth.token-previous-refresh-reuse` [normal] 旧 refresh 触发重用检测后判过期
- `auth.strict-rotation-refresh-replay` [normal,weak,offline,dns,blackhole] strict_rotation 下刷新重放
- `auth.strict-rotation-concurrent` [normal,weak] strict_rotation 并发只刷新一次
- `auth.strict-rotation-two-rounds` [normal] strict_rotation 连续两轮
- `auth.garbage-then-relogin` [normal] 假令牌失败后重新登录能恢复
- `auth.network-failure-keeps-session` [offline,blackhole] 网络故障期间失败，不清 token，恢复后继续可用
- `auth.revoke-all-expired` [normal,weak] revoke scope=all：踢下线后判会话过期
- `auth.revoke-default-scope-is-all` [normal] revoke 不写 scope 等同 all
- `auth.revoke-all-concurrent` [normal] revoke all 后并发请求全部 expired
- `auth.revoke-all-then-relogin` [normal] revoke all 后重新登录可恢复
- `auth.revoke-all-previous-tokens` [normal] revoke all 后用旧 access + 旧 refresh
- `auth.style-switch-refresh` [normal,weak | http] 运行中从 http 切到 business 后 401 仍触发刷新
- `auth.style-switch-concurrent` [normal | http] 切换风格后并发 401 仍只刷新一次
- `auth.style-switch-expired` [normal | http] 切换风格后刷新失败仍判 expired
- `plugins.single-auth` [normal] auth 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `auth.refresh-window-blocked-expired` [normal] 刷新失败窗口内被阻断的请求同样判 expired
- `auth.expired-multipart-replay` [normal] access 过期 401 后刷新并重放 multipart 上传，文件字节与首发一致

### 重试（retry.json，49 个）

- `retry.500-then-ok` [normal,weak,lossy] 前两次 500 后恢复
- `retry.exhausted` [normal,weak] 重试耗尽后返回最后一次错误
- `retry.4xx-no-retry` [normal,weak] 4xx 不重试
- `retry.429-retried` [normal,weak,lossy,offline,blackhole,flap,dns] 429 属于可重试状态
- `retry.503-retry-after` [normal,weak,lossy,offline,blackhole,flap,dns] 遵守 Retry-After
- `retry.post-default-no-retry` [normal,offline] POST 默认不重试
- `retry.post-opt-in-body-intact` [normal,weak,lossy] POST 显式允许重试且请求体一致
- `retry.put-delete-retried` [normal] PUT/DELETE 属于幂等方法，默认重试
- `retry.per-request-max` [normal] 请求级 max 覆盖插件默认
- `retry.per-request-disable` [normal] 请求级 retry:false 关闭重试
- `retry.business-code-not-retried` [normal] HTTP 200 业务失败默认不重试
- `retry.fail-every-all-recover` [normal] 周期故障每次都能被重试掩盖
- `retry.fail-every-no-retry` [normal] 无重试时周期故障原样暴露
- `retry.disabled-by-default` [normal] max=0 即不重试
- `retry.delay-respected` [normal] 重试间隔被遵守
- `retry.cancel-during-wait` [normal] 退避等待中取消立即生效
- `retry.cancel-all-during-wait` [normal] 退避等待中 cancel_all
- `retry.custom-status-list` [normal] 自定义可重试状态
- `retry.parallel-independent` [normal,weak,lossy] 并行请求各自独立重试
- `retry.parse-error-not-retried` [normal] 解析错误不重试
- `retry.network-error-retried` [normal] close 型网络错误可重试
- `retry.post-network-no-retry` [normal] POST 遇网络错误同样不重试
- `retry.hang-timeout-retried` [normal] 超时可重试且每次都真正发出
- `retry.offline-exhausts` [offline,dns] 断网时重试耗尽以 network 失败
- `retry.blackhole-timeout` [blackhole] 黑洞下重试耗尽以 timeout 失败
- `retry.flap-recovers` [flap,normal] 断网恢复后重试成功
- `retry.flap-no-retry-then-ok` [flap] 无重试时断网期间失败、恢复后成功
- `retry.lossy-converges` [lossy,weak] 丢包网络下重试最终收敛
- `retry.attempt-is-new-connection-friendly` [normal] 大量重试后无在途残留
- `retry.respects-retry-after-over-delay` [normal,weak,lossy,offline,blackhole,flap,dns] Retry-After 优于较小的自身退避
- `retry.401-goes-refresh-first` [normal] 重试途中遇 401 走刷新而不是继续重试
- `retry.style-switch` [normal | http] 运行中切换风格后重试仍生效
- `retry.fail-on-noncontiguous-retried` [normal,weak,lossy] fail_on 非连续失败被重试掩盖
- `retry.fail-on-noncontiguous-no-retry` [normal] fail_on 非连续失败无重试原样暴露
- `retry.fail-on-union-fail-times` [normal] fail_on 与 fail_times 取并集
- `retry.fail-on-gap-exhausts` [normal] fail_on 连续失败超过重试次数
- `plugins.single-retry` [normal] retry 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.boundary-retry-max-0` [normal] retry.max = 0、retry.delay_ms = 0（合并等价缺省配置）
- `plugins.boundary-retry-max-1` [normal] retry.max = 1
- `plugins.boundary-retry-max-2` [normal] retry.max = 5
- `plugins.boundary-retry-delay-ms-1` [normal] retry.delay_ms = 1
- `plugins.boundary-retry-methods-0` [normal] retry.methods = []
- `plugins.boundary-retry-methods-1` [normal] retry.methods = [GET]
- `retry.single-req-max-delay` [normal | http] retry 单独：请求级 max 与 delay_ms 覆盖插件默认
- `retry.single-req-methods` [normal | http] retry 单独：请求级 methods 单次放行 POST
- `retry.single-plugin-methods` [normal | http] retry 单独：插件级 methods 只放行 POST
- `retry.single-retry-after-honored` [normal | http] retry 单独：respect_retry_after=true 时按 Retry-After 等待
- `retry.single-retry-after-ignored` [normal | http] retry 单独：respect_retry_after=false 时忽略 Retry-After
- `retry.single-req-retry-after-override` [normal | http] retry 单独：请求级 respect_retry_after 覆盖插件默认

### 缓存（cache.json，32 个）

- `cache.hit-no-server` [normal,weak,offline,dns,blackhole] 命中缓存不到服务器
- `cache.ttl-expiry` [normal,weak] TTL 过期后重新请求
- `cache.http-error-not-cached` [normal] HTTP 错误不入缓存
- `cache.business-failure-not-cached` [normal] 业务失败不入缓存
- `cache.failure-then-success-cached` [normal] 先失败后成功，成功后才缓存
- `cache.diff-query-no-mixup` [normal,weak] 不同查询不串缓存
- `cache.diff-body-no-mixup` [normal] 不同请求体不串 key
- `cache.post-not-cached-by-default` [normal] POST 默认不缓存
- `cache.clear` [normal] clear_cache 后重新请求
- `cache.per-request-disable` [normal] 请求级 cache:false 不读不写
- `cache.per-request-ttl` [normal] 请求级 ttl 覆盖插件默认
- `cache.custom-key-shared` [normal] 自定义缓存 key 让不同查询共享
- `cache.user-scope-no-leak` [normal,weak] 切换用户不串缓存
- `cache.user-scope-same-user-hits` [normal] 同一用户仍能命中缓存
- `cache.offline-serves-cache` [normal] 断网时命中缓存仍可用
- `cache.offline-miss-then-recover` [normal] 断网未命中失败，恢复后正常
- `cache.cancel-not-cached` [normal] 被取消的请求不入缓存
- `cache.large-response` [normal] 大响应缓存
- `cache.max-entries-evicts` [normal] 容量上限淘汰最旧条目
- `cache.timeout-not-cached` [normal] 超时不入缓存
- `cache.concurrent-no-share-all-miss` [normal] 无 share 时并发同 key 各自请求
- `plugins.single-cache` [normal] cache 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.boundary-cache-ttl-ms-0` [normal] cache.ttl_ms = 0
- `plugins.boundary-cache-ttl-ms-1` [normal] cache.ttl_ms = 1
- `plugins.boundary-cache-ttl-ms-2` [normal] cache.ttl_ms = 2147483647
- `plugins.boundary-cache-max-0` [normal] cache.max = 0
- `plugins.boundary-cache-max-1` [normal] cache.max = 1
- `plugins.boundary-cache-max-2` [normal] cache.max = 64
- `plugins.boundary-cache-methods-0` [normal] cache.methods = []
- `plugins.boundary-cache-methods-1` [normal] cache.methods = [GET]
- `cache.single-explicit-key-expires` [normal] cache 单独：请求级显式 key 共享缓存，请求级 ttl 到期失效
- `cache.single-clear-cache` [normal] cache 单独：clear_cache 后同 key 重新回源

### 请求合并（share.json，21 个）

- `share.start-single-flight` [normal,weak,offline,dns,blackhole] 同 key 并发只发一次
- `share.start-32` [normal] 32 个并发同 key 只发一次
- `share.different-keys-independent` [normal,weak] 不同 key 互不合并
- `share.no-cache-after-settle` [normal,weak] 完成后的新请求重新发
- `share.no-key-no-share` [normal] 没设置 key 的请求不合并
- `share.per-request-disable` [normal] 请求级 share:false 不参与合并
- `share.leader-failure-all-fail` [normal,weak] 首发失败时所有跟随者各自拿到错误
- `share.network-failure-all-fail` [offline,dns,blackhole] 网络失败分发给全部跟随者
- `share.follower-cancel-isolated` [normal] 跟随者单独取消不影响首发
- `share.leader-cancel-followers-survive` [normal] 首发取消不连累跟随者
- `share.all-cancelled-aborts-server` [normal] 全部 caller 取消才掐断连接
- `share.end-last-wins` [normal] end 策略：后到者顶替，全员拿最后结果
- `share.race-first-success` [normal] race 策略：第一个成功者分发给所有人
- `share.race-first-fails-second-wins` [normal] race 策略：一个失败另一个成功仍成功
- `share.none-policy` [normal] none 策略等同关闭
- `share.reuse-after-failure` [normal] 失败后同 key 新请求不被旧失败污染
- `plugins.single-share` [normal] share 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `share.single-start-explicit-key` [normal] share 单独：显式 key 下 start 策略只发一次
- `share.single-req-policy-race` [normal] share 单独：请求级 policy=race 覆盖插件默认 start
- `share.single-all-timeout-aborts-server` [normal] share 单独：所有订阅者都超时后掐断底层请求
- `share.retry-recovers-single-result` [normal] share(retry)：首发失败后内部重试，3 个调用方只看到最终的成功结果

### 并发（concurrency.json，16 个）

- `conc.barrier-asc` [normal,weak,offline,dns,blackhole] 顺序放行
- `conc.barrier-desc` [normal,weak] 倒序放行
- `conc.barrier-given` [normal,weak] 指定顺序放行
- `conc.barrier-shuffle` [normal,weak] 乱序放行
- `conc.barrier-all-fail` [normal,weak] 整组失败
- `conc.barrier-partial-fail` [normal,weak] 部分失败互不影响
- `conc.barrier-32-peak` [normal] 32 并发峰值
- `conc.barrier-64-shuffle` [normal] 64 并发乱序
- `conc.barrier-64-desc-order` [normal] 64 并发倒序落定顺序
- `conc.barrier-reuse` [normal] 同一组放行后可复用
- `conc.barrier-partial-wait` [normal] 等不齐时超时放行
- `conc.parallel-failure-isolated` [normal,weak] 并发中的失败互不影响
- `conc.fast-slow-settle-order` [normal,weak] 快慢请求按完成先后落定
- `conc.no-share-peak-inflight` [normal] 无 share 时并发请求真并行
- `conc.fire-then-settle` [normal,weak] fire 的请求在场景结束前落定
- `conc.barrier-mixed-with-other` [normal] 栅栏请求与普通请求混跑

### 取消（cancel.json，13 个）

- `cancel.single` [normal,weak,offline,dns,blackhole] 单个请求取消
- `cancel.hang-is-canceled-not-timeout` [normal] 挂起的请求被取消归为 canceled
- `cancel.all` [normal] cancel_all 取消全部在途请求
- `cancel.all-then-new-works` [normal] cancel_all 之后新请求正常
- `cancel.one-of-many` [normal] 只取消其中一个
- `cancel.after-complete-noop` [normal,weak] 完成后再取消是空操作
- `cancel.all-with-nothing-inflight` [normal] 没有在途请求时 cancel_all 不报错
- `cancel.double-cancel` [normal] 重复取消幂等
- `cancel.slow-body` [normal] 响应体传输中取消
- `cancel.before-reach-server` [normal] 发出即取消
- `cancel.parallel-32` [normal] 32 个请求同时取消
- `cancel.blackhole-cancel-before-timeout` [blackhole] 黑洞下取消先于超时
- `plugins.single-cancel` [normal] cancel 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。

### 加载状态（loading.json，21 个）

- `loading.single-pair` [normal,weak,offline,dns,blackhole] 单个请求一对 show/hide
- `loading.concurrent-single-pair` [normal,weak,offline,dns,blackhole] 并发多个请求只有一对
- `loading.big-parallel-20` [normal] 20 并发依旧一对
- `loading.sequential-pairs` [normal] 串行两个请求两对
- `loading.failure-closes` [normal,weak] 失败也要关闭 loading
- `loading.timeout-closes` [normal] 超时关闭 loading
- `loading.cancel-closes` [normal] 取消关闭 loading
- `loading.retry-no-flicker` [normal,weak,lossy] 重试期间 loading 不闪
- `loading.retry-exhausted-closes` [normal] 重试耗尽后关闭
- `loading.flap-recovers-single-pair` [flap,normal] 断网恢复期间 loading 保持一对
- `loading.delay-fast-never-shows` [normal,offline,dns,blackhole] delay 内完成不显示
- `loading.delay-slow-shows` [normal,weak] 超过 delay 才显示
- `loading.delay-close-merges` [normal,weak] delayClose 合并相邻请求
- `loading.per-request-off` [normal,offline] 请求级 loading:false 不触发
- `loading.mixed-silent-and-normal` [normal] 静默与普通请求混合
- `loading.parallel-partial-fail` [normal] 并发中部分失败仍一对
- `loading.network-switch-pairs` [normal] 断网又恢复各自成对
- `plugins.single-loading` [normal] loading 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.boundary-loading-delay-ms-0` [normal] loading.delay_ms = 0、loading.delay_close_ms = 0（合并等价缺省配置）
- `plugins.boundary-loading-delay-ms-1` [normal] loading.delay_ms = 1
- `plugins.boundary-loading-delay-close-ms-1` [normal] loading.delay_close_ms = 1

### 通知与信封（notify-normalize.json，22 个）

- `notify.http-error-once` [normal,weak,offline,dns,blackhole] HTTP 失败只通知一次
- `notify.silent` [normal,offline] silent 请求不通知
- `notify.success-no-notify` [normal,weak] 成功不通知
- `notify.business-error-once` [normal] 业务失败通知一次
- `notify.retry-recovered-silent` [normal,weak,lossy] 重试恢复后不通知
- `notify.retry-exhausted-once` [normal] 重试耗尽只通知一次
- `notify.parallel-failures-each` [normal] 并发各自失败各自通知
- `notify.cancel-not-notified` [normal] 主动取消不通知
- `notify.parse-error-once` [normal] 解析错误通知一次
- `notify.timeout-once` [normal] 超时通知一次
- `normalize.envelope-unwrap` [normal,weak,offline,dns] 成功信封解包
- `normalize.business-fail` [normal,weak] 业务失败归类 business:<code>
- `normalize.business-negative-code` [normal] 负数业务码
- `normalize.business-multi-codes` [normal] 多个业务码分别归类
- `normalize.http-error-wins` [normal] HTTP 错误优先于业务码
- `normalize.no-envelope-passthrough` [normal,weak] 无信封响应透传
- `normalize.per-request-off` [normal] 请求级 normalize:false 不判业务码
- `normalize.business-notify-retry-combo` [normal] 业务失败：不重试、通知一次
- `normalize.style-switch` [normal | http] 运行中切换风格后错误分类不变
- `plugins.single-normalize` [normal] normalize 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.single-notify` [normal] notify 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `notify.single-silent-and-stringify` [normal | http] notify 单独：失败提示一次，silent 请求与成功请求不提示

### 故障响应（faults.json，16 个）

- `faults.hang-timeout` [normal,weak] hang 归为 timeout 并掐断连接
- `faults.close-network` [normal,weak] close 归为 network
- `faults.half-body` [normal,weak] half_body 归为 network 或 parse
- `faults.bad-json` [normal,weak] bad_json 归为 parse
- `faults.empty-body` [normal,weak] 空 body 的 200
- `faults.html-gateway-page` [normal,weak] 网关 HTML 拦截页
- `faults.html-502` [normal,weak | http] 502 的 HTML 页归为 status:502
- `faults.slow-body-succeeds` [normal,weak] slow_body 慢但成功
- `faults.large-size` [normal] 大响应
- `faults.no-envelope` [normal,weak] 裸 JSON 响应
- `faults.close-parallel` [normal] 并发 close 全部落定
- `faults.hang-parallel-timeouts` [normal] 并发 hang 同时超时
- `faults.recovery-after-bad` [normal,weak] 坏响应之后客户端仍正常
- `faults.post-close-no-double-send` [normal] POST 遇 close 不重复发送
- `faults.status-with-business` [normal,weak,lossy,offline,blackhole,flap,dns] 非 2xx 且带业务码
- `faults.non2xx-business-safe` [normal,weak] HTTP 500 与非零业务码的分类优先级

### 网络条件（network.json，14 个）

- `network.offline-immediate` [offline,dns] 断网立即以 network 失败
- `network.offline-all-methods` [offline,dns] 断网下各方法一致失败
- `network.blackhole-timeout` [blackhole] 黑洞靠超时发现
- `network.blackhole-parallel-timeouts` [blackhole] 黑洞下并发同时超时
- `network.blackhole-cancel-all` [blackhole] 黑洞下 cancel_all 立即生效
- `network.flap-no-retry` [flap] 断网窗口内失败、恢复后成功
- `network.flap-retry-recovers` [flap,normal] flap + retry 恢复
- `network.flap-parallel-retry` [flap] flap 下并发请求全部恢复
- `network.switch-offline-online` [normal] 运行中断网再恢复
- `network.lossy-parallel-retry` [lossy,weak] 丢包网络下并发重试收敛
- `network.weak-parallel` [normal,weak] 弱网并发成功
- `network.offline-share-loading` [offline,dns] 断网下 share + loading
- `network.recovery-state-clean` [normal] 断网失败后同 key 恢复正常
- `network.dns-failure` [dns] DNS 失败归为 network

### 超时（dioman）（timeout.json，20 个）

- `timeout.total-hang` [normal] 总超时：hang 到点即超时
- `timeout.receive-slow-body` [normal] 接收超时：块间隔过大
- `timeout.receive-ok-fast-chunks` [normal] 接收超时不误伤正常流
- `timeout.total-over-slow-chunks` [normal] 总超时覆盖慢吐 body
- `timeout.connect-blackhole` [blackhole] 黑洞下由接收超时终止（已连接，不是连接超时）
- `timeout.request-override-shorter` [normal] 请求级超时更短
- `timeout.request-override-longer` [normal] 请求级超时更长
- `timeout.within-budget` [normal] 预算内的慢响应成功
- `timeout.recovers-after-timeout` [normal] 超时之后客户端仍正常
- `timeout.parallel-all-timeout` [normal] 并发超时并行计时
- `timeout.weak-generous-budget` [normal,weak] 弱网下宽松预算成功
- `timeout.total-slow-chunks-small` [normal] 总超时：小块慢吐
- `timeout.total-budget-slow-body-ok` [normal] 总超时预算足够时慢吐 body 成功
- `timeout.receive-gap-vs-chunk-bytes` [normal] 接收超时看块间隔，不看块大小
- `plugins.single-timeout` [normal] timeout 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.boundary-timeout-connect-ms-0` [normal] timeout.connect_ms = 1
- `plugins.boundary-timeout-connect-ms-1` [normal] timeout.connect_ms = 2147483647
- `plugins.boundary-timeout-receive-ms-0` [normal] timeout.receive_ms = 1
- `plugins.boundary-timeout-receive-ms-1` [normal] timeout.receive_ms = 2147483647
- `timeout.send-slow-reader` [normal] 发送超时：对端迟迟不读请求体，send_ms 必须在发送阶段生效

### 熔断（dioman）（breaker.json，19 个）

- `breaker.open-after-threshold` [normal,weak] 连续失败后熔断，快速失败不打服务端
- `breaker.half-open-recovers` [normal] open_ms 后半开探测成功则恢复
- `breaker.half-open-fail-reopens` [normal] 半开探测失败重新熔断
- `breaker.success-resets-count` [normal] 成功会重置连续失败计数
- `breaker.bucket-isolation` [normal] 熔断按接口隔离
- `breaker.per-request-skip` [normal] 请求级 breaker:false 跳过熔断
- `breaker.timeouts-count` [normal] 超时计入失败
- `breaker.4xx-not-counted` [normal] 普通 4xx 不触发熔断
- `breaker.network-errors-trip` [offline,dns] 网络错误累计后熔断
- `breaker.retry-halts-on-trip` [normal] 重试在熔断后立刻停止
- `breaker.parallel-trip` [normal] 并发失败触发熔断
- `breaker.open-fast-fail` [normal,weak] 熔断状态下失败几乎零耗时
- `plugins.single-breaker` [normal] breaker 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.boundary-breaker-threshold-0` [normal] breaker.threshold = 1
- `plugins.boundary-breaker-threshold-1` [normal] breaker.threshold = 64
- `plugins.boundary-breaker-open-ms-0` [normal] breaker.open_ms = 0
- `plugins.boundary-breaker-open-ms-1` [normal] breaker.open_ms = 1
- `plugins.boundary-breaker-open-ms-2` [normal] breaker.open_ms = 2147483647
- `breaker.half-open-probe-cancel` [normal] 半开探测在途被取消：名额归还，不计成败，下一个探测照常放行

### 离线（dioman）（offline.json，9 个）

- `offline.probe-fails-fast` [offline,dns,blackhole] 探测判无网直接 offline 错误
- `offline.online-passes` [normal,weak] 在线时放行
- `offline.recovers-after-online` [offline] 恢复在线后放行
- `offline.flap-recovers` [flap] 断网窗口内 offline，恢复后成功
- `offline.parallel-all-offline` [offline] 并发请求全部 offline
- `offline.cache-hit-still-served` [normal] 缓存命中不受离线影响
- `offline.cancel-while-probing` [blackhole] 探测期间取消
- `offline.then-online-no-stale-state` [offline] 离线失败后不残留状态
- `plugins.single-offline` [normal] offline 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。

### 路径/键/过滤（repath-key-filter.json，18 个）

- `repath.basic` [normal,weak] 路径变量替换
- `repath.multi-vars` [normal] 多个路径变量
- `repath.from-body` [normal] 变量取自请求体
- `repath.zero-value` [normal] 值为 0 也要替换
- `repath.same-var-twice` [normal] 同名变量出现两次
- `key.share-diff-query-not-merged` [normal] 查询不同不合并
- `key.query-order-insensitive` [normal] 查询参数顺序不影响 key
- `key.body-diff-not-merged` [normal] 请求体不同不合并
- `key.method-diff-not-merged` [normal] 方法不同不合并
- `key.explicit-string-merges` [normal] 显式字符串 key 强制合并
- `key.filter-before-key` [normal] filter 先于 key：空值不影响 key
- `filter.keeps-zero-false` [normal,weak] filter 不误删 0 和 false
- `key.share-without-key-not-merged` [normal] share 缺 key 不合并
- `repath.key-after-repath` [normal] repath 先于 key
- `repath.key-cache-no-mixup` [normal,weak] repath + key + cache 不串
- `plugins.single-filter` [normal] filter 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.single-key` [normal] key 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `plugins.single-repath` [normal] repath 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。

### 组合交互（combos.json，33 个）

- `combo.cache-share-retry` [normal,weak,lossy] cache + share + retry
- `combo.share-failure-not-cached` [normal] share 失败不入缓存
- `combo.auth-retry-loading-notify-recovers` [normal,weak,lossy] auth + retry + loading + notify：一次逻辑请求
- `combo.auth-retry-loading-notify-fails` [normal] auth + retry + loading + notify：最终失败
- `combo.auth-concurrent-loading-notify` [normal,weak] auth + loading + notify 并发刷新
- `combo.auth-expired-loading-notify` [normal] auth 会话过期时的 loading/notify
- `combo.auth-cache-refresh` [normal] auth + cache：刷新后缓存仍可用
- `combo.cancel-share-follower` [normal] cancel + share：跟随者取消
- `combo.cancel-all-share` [normal] cancel_all + share
- `combo.retry-cancel-loading-notify` [normal] retry + cancel + loading + notify
- `combo.share-retry-loading-notify` [normal,weak,lossy] share + retry + loading + notify
- `combo.share-fail-notify` [normal] share + notify：并发失败的提示
- `combo.loading-cache-hit-delay` [normal] loading + cache：命中不闪 loading
- `combo.normalize-business-retry-notify-cache` [normal] 业务失败 + retry + notify + cache
- `combo.lossy-everything` [lossy,weak,normal] 丢包下全插件组合
- `combo.flap-share-cache-loading` [flap] flap 下 share + cache + loading + retry
- `combo.offline-share-notify` [offline,dns] 离线 + share + notify
- `combo.full-stack-happy` [normal,weak] 全家桶 happy path
- `combo.all-seventeen-plugins` [normal] 全部 17 个插件可共同安装和释放
- `combo.cache-mock-no-key-collision` [normal] cache + mock：mock 响应不进缓存，也不污染同 key 的真实请求
- `combo.cache-mock-fallback-cached-once` [normal | http] cache + mock：404 回落真实接口，回落的最终结果被缓存，后续回落直接命中缓存
- `combo.loading-mock-fallback-single-pair` [normal | http] loading + mock：回落到真实接口只算一对 loading
- `combo.mock-share-fallback-single-flight` [normal | http] mock + share：回落发生在共享窗口内不死锁，回落的真实请求只发一次
- `combo.auth-envs-refresh-replay` [normal] auth + envs：装配 envs 后刷新重放仍带新凭证
- `combo.auth-logger-refresh-deny-revoked` [normal] auth + logger：刷新、越权、踢下线三条路径在装配 logger 后行为不变
- `combo.cache-envs-hit-and-ttl` [normal] cache + envs：装配 envs 后缓存命中与 TTL 过期照常
- `combo.cancel-logger-cleanup` [normal] cancel + logger：取消路径装配 logger 后仍能清理并继续工作
- `combo.envs-mock-routing` [normal] envs + mock：装配 envs 后 mock 路由与真实路由互不串
- `combo.filter-repath-path-var-first` [normal] filter + repath：路径变量先被消费，不受过滤影响
- `combo.envs-retry-recovers-and-exhausts` [normal] envs + retry：装配 envs 后重试恢复与耗尽的次数照旧
- `combo.filter-retry-recovers-and-exhausts` [normal] filter + retry：装配 filter 后重试恢复与耗尽的次数照旧
- `combo.logger-retry-recovers-and-exhausts` [normal] logger + retry：装配 logger 后重试恢复与耗尽的次数照旧
- `combo.repath-retry-path-stable` [normal] repath + retry：重试重放时路径变量不丢

### 模拟（mock.json，8 个）

- `mock.routes-to-mock-server` [normal,weak,offline,dns,blackhole] 请求被路由到 /t/mock
- `mock.per-request-off-goes-real` [normal,weak] 请求级 mock:false 走真实接口
- `mock.mixed-parallel` [normal] mock 与真实请求并行不串
- `mock.query-and-body-forwarded` [normal] mock 请求保留查询和请求体
- `mock.fallback-to-real-on-404` [normal | http] mock 返回 404 时回落到真实接口且只回落一次
- `mock.mock-then-retry-untouched` [normal] mock 请求成功时 retry 不介入
- `plugins.single-mock` [normal] mock 单独启用：成功、500、主动取消；不代表全部公开选项已覆盖。
- `mock.single-fallback-disabled` [normal | http] mock 单独：fallback_on_404=false 时 404 不回落

### filter（filter.json，3 个）

- `filter.drops-empty-body-keeps-zero-false` [normal,weak] filter 开启：请求体顶层剔除 null/空白字符串，保留 0/false
- `filter.drops-blank-query-keeps-zero-false` [normal,weak] filter 开启：查询参数里的空白字符串被剔除，0/false 保留
- `filter.off-by-default-sends-as-is` [normal,weak] 未开启 filter：已装配插件也原样发送

### plugins（plugins.json，34 个）

- `plugins.pair-0` [normal] 二元正交数组第 0 行；17 个开关（含全部关闭基线）。
- `plugins.pair-1` [normal] 二元正交数组第 1 行；17 个开关（含全部关闭基线）。
- `plugins.pair-2` [normal] 二元正交数组第 2 行；17 个开关（含全部关闭基线）。
- `plugins.pair-3` [normal] 二元正交数组第 3 行；17 个开关（含全部关闭基线）。
- `plugins.pair-4` [normal] 二元正交数组第 4 行；17 个开关（含全部关闭基线）。
- `plugins.pair-5` [normal] 二元正交数组第 5 行；17 个开关（含全部关闭基线）。
- `plugins.pair-6` [normal] 二元正交数组第 6 行；17 个开关（含全部关闭基线）。
- `plugins.pair-7` [normal] 二元正交数组第 7 行；17 个开关（含全部关闭基线）。
- `plugins.pair-8` [normal] 二元正交数组第 8 行；17 个开关（含全部关闭基线）。
- `plugins.pair-9` [normal] 二元正交数组第 9 行；17 个开关（含全部关闭基线）。
- `plugins.pair-10` [normal] 二元正交数组第 10 行；17 个开关（含全部关闭基线）。
- `plugins.pair-11` [normal] 二元正交数组第 11 行；17 个开关（含全部关闭基线）。
- `plugins.pair-12` [normal] 二元正交数组第 12 行；17 个开关（含全部关闭基线）。
- `plugins.pair-13` [normal] 二元正交数组第 13 行；17 个开关（含全部关闭基线）。
- `plugins.pair-14` [normal] 二元正交数组第 14 行；17 个开关（含全部关闭基线）。
- `plugins.pair-15` [normal] 二元正交数组第 15 行；17 个开关（含全部关闭基线）。
- `plugins.pair-16` [normal] 二元正交数组第 16 行；17 个开关（含全部关闭基线）。
- `plugins.pair-17` [normal] 二元正交数组第 17 行；17 个开关（含全部关闭基线）。
- `plugins.pair-18` [normal] 二元正交数组第 18 行；17 个开关（含全部关闭基线）。
- `plugins.pair-19` [normal] 二元正交数组第 19 行；17 个开关（含全部关闭基线）。
- `plugins.pair-20` [normal] 二元正交数组第 20 行；17 个开关（含全部关闭基线）。
- `plugins.pair-21` [normal] 二元正交数组第 21 行；17 个开关（含全部关闭基线）。
- `plugins.pair-22` [normal] 二元正交数组第 22 行；17 个开关（含全部关闭基线）。
- `plugins.pair-23` [normal] 二元正交数组第 23 行；17 个开关（含全部关闭基线）。
- `plugins.pair-24` [normal] 二元正交数组第 24 行；17 个开关（含全部关闭基线）。
- `plugins.pair-25` [normal] 二元正交数组第 25 行；17 个开关（含全部关闭基线）。
- `plugins.pair-26` [normal] 二元正交数组第 26 行；17 个开关（含全部关闭基线）。
- `plugins.pair-27` [normal] 二元正交数组第 27 行；17 个开关（含全部关闭基线）。
- `plugins.pair-28` [normal] 二元正交数组第 28 行；17 个开关（含全部关闭基线）。
- `plugins.pair-29` [normal] 二元正交数组第 29 行；17 个开关（含全部关闭基线）。
- `plugins.pair-30` [normal] 二元正交数组第 30 行；17 个开关（含全部关闭基线）。
- `plugins.pair-31` [normal] 二元正交数组第 31 行；17 个开关（含全部关闭基线）。
- `plugins.full-workflow` [normal,weak] 17 插件全开：登录→访问→刷新→重试→缓存→共享→取消→通知。
- `plugins.full-flap-workflow` [flap] 17 插件全开：flap 离线失败后恢复，再执行完整业务流。
