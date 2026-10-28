# 提议：补齐 ② 档真实核对缺口（来自 dioman，2026-10-28）

> 状态：**已落地**（2026-10-28，用户（jelon）授权，主会话代拍；决策见 D20–D23）。**只有 §6（connect 超时）和"其他 A"里的 HTTP 日期形式 Retry-After 未落地**，原因见各节。下面保留提议原文作为历史，当前以 `CONTRACT.md`、`scenario.schema.json`、`scenarios/` 为准。
>
> **各节去向**
>
> | 节 | 去向 | 决策 | 契约 / schema | 场景 |
> |---|---|---|---|---|
> | §1 熔断半开探测被取消 | 已落地 | D23 | 无改动 | `breaker.half-open-probe-cancel` |
> | §2 multipart 重放 | 已落地 | D23 | CONTRACT §7（multipart 请求体、seed 的 LCG 算法与自检向量）、§4.1/附录 A（echo 回报）；schema `request.body`、`$defs/multipartBody`；能力 `multipart` | `auth.expired-multipart-replay` |
> | §3 连接复用 | 已落地 | D23 | CONTRACT §2（`meta.conn`）、§7（`same_as` 与 `keep-alive`）；schema `result.same_as`；能力 `keep-alive` | `basic.keep-alive-recovery-isolated` |
> | §4 share retry | 已落地 | D23 | CONTRACT §7（`share.policy: retry`）；schema policy 与 `options.share`；能力 `share-retry` | `share.retry-recovers-single-result` |
> | §5 send 超时 | 已落地（请求体取 16MiB） | D22 | CONTRACT §3（`read_delay_ms`）、§7（`timeout.send_ms`）；schema `pluginOptions.timeout`；能力 `timeout-send` | `timeout.send-slow-reader` |
> | §6 connect 超时 | **未落地**：netem 丢 SYN 不可行 | D22 | – | 不加 `timeout.connect-nosyn`；`timeout.connect-blackhole` 已改标题和 why |
> | 其他 A 去掉 429/503 的 skip | 已落地；HTTP 日期形式的 Retry-After **未落地**（服务端只能输出整数秒） | D20 | CONTRACT §3（`retry_after`）、附录 B.6/B.7 | `retry.429-retried`、`retry.503-retry-after`、`retry.respects-retry-after-over-delay`、`basic.status-matrix`、`faults.status-with-business` |
> | 其他 B mock 组合语义 | 已落地 | D23 | CONTRACT §7（硬要求 + 可选语义）；能力 `mock-in-chain` | `combo.cache-mock-fallback-cached-once`、`combo.mock-share-fallback-single-flight`（requires 加 `mock-in-chain`） |
> | 其他 C dioman 的设计差异 | 随 B 解决（dioman 按缺能力跳过） | D23 | 同上 | 同上 |
> | 其他 D `plugins.single-auth` | 结论：预期 `ok: true` 正确，不改；dioman 侧要跟进 | D21 | – | `plugins.single-auth` 只改了 why |
>
> **未落地的两项，保留原因**
> - §6 connect 超时：服务端上一步的结论是 netem 无法丢 SYN——代理只能 accept 内核已完成握手的连接，停止 accept 也挡不住握手，跨平台无法可靠控制；新增服务端测试证明 blackhole 是握手完成后才读超时。替代方案：connect 超时由客户端单测（pending 连接器）覆盖，黑盒只测接收超时。重新评估的条件见 D22。
> - HTTP 日期形式的 Retry-After：`retry_after` 参数只能输出整数秒，要黑盒验证日期形式需要服务端新增能力（例如 `retry_after_date`），本次没有改服务端；dioman 想核对的日期格式仍只能靠客户端单测。
> 来源：dioman 的测试总账（`E:\workspaces\dart-labs\dioman\test\accept\lib\matrix.json`）把每个能力分成三档，其中"② 单测 + 真实核对"要求至少一个真实场景证明替身的假设成立。下面列的是 dioman 侧找不到任何 testkit 场景能核对的能力。它们对所有执行器都有意义，不是 dioman 特有的。
> 假设编号（H1–H8）的定义见 `E:\workspaces\dart-labs\dioman\test\ASSUMPTIONS.md`。

## TL;DR

| # | 能力 | 需要什么 | 影响其他执行器 |
|---|---|---|---|
| 1 | 熔断半开探测在途被取消后，名额可复用、不计成败 | **只新增场景**，现有 step 足够 | 有 breaker 能力的执行器要跑 |
| 2 | 认证 401 后重放 multipart 请求体 | schema 支持 multipart body；echo 回报文件字节摘要 | 有 auth 能力的执行器要能发 multipart |
| 3 | 连接复用（keep-alive）下恢复失败不污染后续响应 | 服务端在 `meta` 回报连接标识 | 只读断言，执行器不用改 |
| 4 | share 的 `retry` 策略 | CONTRACT §7 share.policy 增加 `retry` | 没有该策略的执行器映射为"缺能力"跳过 |
| 5 | send 超时真正生效 | schema `timeout.send_ms`；服务端"慢读请求体"控制参数 | 有 timeout 能力的执行器要映射 send_ms |
| 6 | connect 超时真正生效 | netem 增加"丢 SYN"档（不完成握手） | 只加网络档，执行器不用改 |

另有三件事与上面无关，但同样需要 testkit 处理，见文末"其他"。

---

## 1. 熔断半开探测在途被取消（只新增场景）

> 已落地：`breaker.half-open-probe-cancel`（D23），场景与下面草案基本一致，另补了 f1–f3 的 `status:500` 预期。

**为什么需要**：半开状态只放有限个探测请求。如果探测请求在途被用户取消，熔断器必须**归还名额**，并且**不把取消算成成功或失败**。否则会出现两种坏结果：名额被永久占住，之后一直拒绝；或者一次取消就把熔断器关上或重新打开。单测用的是假传输，取消时的竞态需要真实网络来核对（H1）。

**建议场景**（放 `scenarios/breaker.json`）：

```json
{
  "id": "breaker.half-open-probe-cancel",
  "title": "半开探测在途被取消：名额归还，不计成败，下一个探测照常放行",
  "why": "取消不反映依赖健康；不能占住半开名额，也不能把熔断器关上或重新打开",
  "requires": ["breaker", "normalize"],
  "networks": ["normal"],
  "client": {"plugins": {"breaker": {"threshold": 3, "open_ms": 1000}, "normalize": {}}},
  "steps": [{"sequence": [
    {"request": {"id": "f1", "path": "/t/public/bk", "query": {"status": 500}}},
    {"request": {"id": "f2", "path": "/t/public/bk", "query": {"status": 500}}},
    {"request": {"id": "f3", "path": "/t/public/bk", "query": {"status": 500}}},
    {"request": {"id": "blocked", "path": "/t/public/bk"}},
    {"wait": 1300},
    {"request": {"id": "probe", "path": "/t/public/bk", "query": {"delay_ms": 3000}, "cancel_after_ms": 300}},
    {"request": {"id": "probe2", "path": "/t/public/bk"}},
    {"request": {"id": "after", "path": "/t/public/bk"}}
  ]}],
  "expect": {"normal": {
    "results": {
      "blocked": {"ok": false, "error": "breaker_open"},
      "probe": {"ok": false, "error": "canceled"},
      "probe2": {"ok": true},
      "after": {"ok": true}
    },
    "server": {"by_path": {"/t/public/bk": {"eq": 6}}, "aborted": {"min": 1}},
    "client": {"pending_after": {"eq": 0}}
  }}
}
```

计数说明：f1–f3 共 3 次，probe 到达服务端后被中止 1 次，probe2 和 after 各 1 次，合计 6 次；`blocked` 被熔断器拦下，不到服务端。

## 2. 认证 401 后重放 multipart 请求体（需改 schema 与 echo）

> 已落地：D23。场景用 `expire_local` 触发 401（而不是 `fail_status`），文件 sha256 在场景里写死。seed 算法是 LCG，见 CONTRACT §7。

**为什么需要**：token 过期时上传文件，auth 刷新后要重放原请求。multipart 的请求体是**一次性的流**：第一次发送时已经被读完，重放时必须重新构造（克隆 FormData），否则重放发出的是空体或残缺体，而服务端只会看到"上传成功"（H2）。单测的假传输不读请求体，这个问题测不出来。

**需要**：
- schema：`request.body` 支持 `{"multipart": {"fields": {...}, "files": [{"name", "filename", "size", "seed"}]}}`，文件内容由 seed 确定性生成，各执行器自行构造。
- echo：对 multipart 请求在 `data` 里回报各部分的名字、长度和摘要（如 sha256 前 16 位）。
- 场景草案：`auth.expired-multipart-replay`。先登录，token 立即过期，然后 POST multipart（`fail_status` 401 一次），断言最终 ok，且 echo 回报的文件摘要与发送端一致；`server.by_path` 恰好 2 次（原始 1 次，重放 1 次）。

## 3. 连接复用下恢复失败不污染后续响应（需服务端回报连接标识）

> 已落地：D23。因失败响应拿不到 `data`，场景改成"成功 warm → 失败 → 失败 → 成功 final"，用 `same_as` 比较 warm 与 final 的 `meta.conn`。

**为什么需要**：客户端复用 keep-alive 连接时，如果一次失败响应（401/500）的响应体没有读完，下一个复用这条连接的请求可能读到**上一次的残留字节**。dioman 本地测试为了规避这个问题，一直强制短连接（H4）。dioman 已经在本地真实回环上核对过（服务端给出 content-length 时没有问题），但还缺少真实服务和真实网络下的证据，而且其他执行器的 HTTP 栈各不相同。

**需要**：
- 服务端在每个响应的 `meta` 里加 `conn`（服务端为每条 TCP 连接分配的序号）。
- 场景草案：`basic.keep-alive-recovery-isolated`。依次请求 401（auth 恢复失败）、500（retry 耗尽）、200，断言三次的 `meta.conn` 相同（确实复用了连接），且最后一个 200 的数据完整、没有被污染。不支持连接复用的执行器，用 `skip` 写明原因。

## 4. share 的 retry 策略（需改 CONTRACT §7）

> 已落地：D23。

**为什么需要**：中立契约的 share.policy 只有 start/end/race。dioman（以及常见的去重实现）还有 `retry`：共享同一个结果，失败时在内部重试，调用方只看到最终的一次结果。没有契约就没有场景，这个策略在所有执行器上都得不到真实验证（H2、H7）。

**需要**：CONTRACT §7 的 share.policy 增加 `retry`，语义写清楚：内部重试次数、间隔、请求体是否重放。没有该策略的执行器按"缺能力"跳过。场景草案：`share.retry-recovers-single-result`，3 个并发、`fail_on=1`，断言 3 个都 ok、`server.by_path` 恰好 2 次。

## 5. send 超时真正生效（需改 schema 与服务端）

> 已落地：D22。请求体取 16MiB（实测 8MB 距离缓冲阈值太近），用 multipart 文件构造。

**为什么需要**：插件只是改写了超时配置，客户端库是否真的在发送阶段计时（H5）只能真实核对。dioman 已经在本地回环上证明 Dart/dio 的 sendTimeout 计的是"请求体写入 socket 的总时长"，只有对端不读、缓冲区塞满时才会触发。各执行器的 HTTP 栈语义不同，需要统一的场景。

**需要**：
- schema：`client.plugins.timeout` 增加 `send_ms`。
- 服务端：新增控制参数 `read_delay_ms`，在开始读请求体之前停顿。
- 场景草案：`timeout.send-slow-reader`。POST 一个大体积请求体（如 8MB，seed 生成），`read_delay_ms: 5000`，`send_ms: 500`，断言 `error: timeout`、`duration_ms` 不超过 2000。不区分发送超时和接收超时的执行器，用 `skip` 写明原因。

## 6. connect 超时真正生效（需改 netem）

> **未落地**：netem 丢 SYN 不可行（D22）。仅"修正 `timeout.connect-blackhole` 的标题和 why"一项已完成。

**为什么需要**：现有的 `timeout.connect-blackhole` 实际测到的是**接收**超时。netem 的 blackhole 档在用户态完成了 TCP 握手，连接是建立成功的。connect 超时只有在对端不回 SYN-ACK 时才会触发（H5）。

**需要**：netem 增加一档 `nosyn`：代理端口不 listen，由防火墙规则或原始 socket 丢弃 SYN；如果用户态做不到，就单独开一个不响应的端口。另外**修正** `timeout.connect-blackhole` 的标题和 why，写明它测的是接收超时。场景草案：`timeout.connect-nosyn`，`connect_ms: 500`，断言 `error: timeout`、`duration_ms` 不超过 2000。

---

## 其他

### A. 去掉注入 429/503 场景里的显式 skip

> 已落地：D20（HTTP 日期形式的 Retry-After 除外，服务端只能输出整数秒）。

`retry.429-retried`、`retry.503-retry-after`、`retry.respects-retry-after-over-delay`（retry.json），以及 `basic.status-matrix`（basic.json）、`faults.status-with-business`（faults.json），在 `expect` 的全部网络下都写了 skip，原因是"本轮用户要求遇 429/503 立即停止"。这是**某一轮的临时约束**，不应当长期留在共享场景里。

dioman 执行器已经改为：遇到 429/503 先读响应体，带服务端注入标记（`data.reason == "injected"`，见 `src/testkit/mod.rs` 的 `data[REASON] = INJECTED`）就放行；只有不带标记的才当作真实限流或熔断，整轮停止。建议把这 5 个场景的 skip 换成真实预期，至少补上 normal 网络。dioman 最想核对的是 Retry-After 的 HTTP 日期格式和 503 路径，目前只有 `retry.single-retry-after-honored`（413 注入、秒数格式）跑通了。

### B. CONTRACT §7 补 mock 的组合语义

> 已落地：D23，能力名 `mock-in-chain`。

`combo.cache-mock-no-key-collision`、`combo.cache-mock-fallback-cached-once`、`combo.mock-share-fallback-single-flight` 依赖的组合语义只写在场景的 `why` 里，引用的还是 axp 的实现（`mock.ts` 的 `mockConfig.cache=false`）：
- mock 的探测请求不走 cache，也不参与 share；
- 只有 404 回落后的真实请求才走 cache/share；
- mock 响应不写缓存，也不污染同一个 key 的真实请求。

建议写成 §7 mock 能力的中立条文。第三条是所有实现都该遵守的硬要求（违反它，关掉 mock 后真实请求会拿到缓存里的假数据）；前两条是"mock 在链上位置"的实现选择，建议标成"可选语义"，见 C。

### C. dioman 的设计差异（已在 dioman 执行器中标为不适用）

> 随 B 解决：这两个场景加了 `mock-in-chain`，dioman 按缺能力跳过（D23）。

dioman 于 2026-10-28 把 mock 从拦截器改为传输层的 `DiomanMockAdapter`（包住真实 adapter，位于整条拦截器链之下）。因此：
- `combo.cache-mock-fallback-cached-once`：回落拿到的真实数据会被缓存，第二次直接命中缓存，不再探测 mock。
- `combo.mock-share-fallback-single-flight`：相同的并发请求先在 share 层被合并，只发一次 mock 探测；场景用"总命中次数"控制 404，合并后计数错位。

dioman 执行器把这两格记为跳过，原因前缀是 `不适用：`。第三条硬要求（mock 响应不进缓存）dioman 由响应标记头 `x-dioman-mock` 加 cache 内置规则保证，`combo.cache-mock-no-key-collision` 照常通过。建议 testkit 在这两个场景上加一个可选能力标记（例如 `requires: ["mock-in-chain"]`），没有这项能力的执行器统一跳过，不必由各执行器各自特判。

### D. `plugins.single-auth` 的 business 预期有未提交改动

> 结论见 D21：`ok: true` 正确（auth 只识别 401/403，一般业务失败靠 normalize），该改动当时已随 9820bdb 提交；需要 dioman 侧改生成器和执行器。

当前工作区的 `scenarios/auth.json` 把 `plugins.single-auth@business` 的预期改成了 `ok: true`，这个改动还没提交。dioman 在改动前后的行为一致：business 风格下的 500 被识别为 `DiomanException(code: 500)`，所以这一格现在失败。如果新预期是有意的（只装 auth、不装 normalize 时不应识别业务失败），请在 DECISIONS 里记一条；dioman 执行器在装了 auth 时也会启用信封转换器，对齐时需要一起调整。
