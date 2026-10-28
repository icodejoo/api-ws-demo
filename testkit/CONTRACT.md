# 客户端验收测试契约（testkit）

这套契约给所有 HTTP 客户端插件库做黑盒验收：axp（TS/axios）已接入，dioman（Dart/dio）后续接入。
三层分工：

```
客户端执行器（各语言自写，读 scenarios/*.json）
   │
   ▼
netem（Rust，独立可执行文件 src/bin/netem.rs）  ← 网络条件：正常 / 弱网 / 断网 / 抖动恢复
   │
   ▼
api-ws-demo 的 /t/* 测试接口                       ← 业务场景：鉴权、失败、并发顺序、坏响应
```

- 场景目录 `scenarios/*.json` 与语言无关，结构见 `scenario.schema.json`。
- 客户端只依赖本文档和 schema，不读服务器源码。
- 本文档就是契约：接口行为改了，先改这里，再改实现。

---

## 1. 运行轮次（隔离）

每个 `/t/*` 请求都要带请求头 **`X-Test-Run: <runId>`**（也可以用查询参数 `run=`）。命中计数、flaky 状态、
barrier 分组、测试用户、token 都按 runId 隔离，不同轮次、不同客户端并行跑不会串数据。

- 不带 runId 时归入 `default` 轮次。
- 轮次 10 分钟无访问后自动回收；也可以主动调用 `DELETE /t/runs/{run}` 删除。

## 2. 统一响应

和全站一致，用信封 `{code, data, message}`；`code = 0` 表示成功。所有 `/t/*` 成功响应的 `data`
都带一段公共字段 `meta`：

```json
{ "meta": { "run": "r1", "key": "k", "hit": 3, "path": "/t/echo", "method": "GET",
            "auth": "u1" , "ts": 1730000000000 } }
```

- `hit` 是同一 (run, key) 下的第几次命中，从 1 开始。key 默认取 method + path，可以用 `key=` 参数覆盖。
- `auth` 是鉴权通过的用户名；匿名请求为 `null`。

### 2.1 响应风格（两种都要支持）

真实后端报告失败有两种常见做法，客户端两种都要能正确处理：

| 风格 | HTTP 状态码 | `code` | 例子：未登录访问受保护接口 |
|---|---|---|---|
| **`http`**（默认） | 标准状态码 | 和 HTTP 状态码一致 | HTTP 401，`{"code":401,...}` |
| **`business`** | **一律是 200** | 原本应返回的状态码 | HTTP 200，`{"code":401,...}` |

- **怎么选**：整轮用请求头 `X-Test-Style: http|business` 指定，单个请求可以用查询参数 `style=` 覆盖。
- **适用范围**：所有 `/t/*` 产生的失败都会按风格输出，包括鉴权失败、越权、`fail_times`/`fail_every` 注入的失败、
  `fail_on`、barrier 的 fail、`/t/auth/refresh` 和 `/t/auth/login` 的失败，以及 `status`/`code` 参数显式指定的失败。
- **不受影响的**：连接层故障（`fault` 里的 hang、close、half_body 等）和 netem 网络条件，它们发生在 HTTP 之下，和风格无关。
- **成功响应**：两种风格完全一样，都是 HTTP 200，`code = 0`。
- **business 风格下的 Retry-After**：失败时附带的 `Retry-After` 头照常输出。
- **细分原因**：同一个 code 下面的具体原因放在 `data.reason` 里，见 §4.2。两种风格下 `data.reason` 都会出现，
  失败响应的 `data` 结构是 `{reason, ...}`，不会是 null。

## 3. 通用控制参数（适用于所有 `/t/*` 业务接口）

全部走查询参数（POST 也一样），这样客户端不用改请求体就能控制服务器怎么响应：

| 参数 | 默认 | 说明 |
|---|---|---|
| `key` | method+path | 命中计数和 flaky 状态的分组键 |
| `delay_ms` | 0 | 响应前先等待这么久，上限 10000 |
| `status` | 200 | 逻辑状态码。http 风格下作为 HTTP 状态码输出；business 风格下 HTTP 为 200，这个值写进 `code` |
| `code` | 同 status，status 为 200 时取 0 | 显式指定业务码，优先级高于 status 推导出的值；HTTP 200 + 非 0 code 用来模拟纯业务失败，比如 `code=1001` 表示库存不足 |
| `style` | 由 `X-Test-Style` 决定 | 本次请求的响应风格：`http` 或 `business`，见 §2.1 |
| `message` | `ok` | 信封里的 message |
| `data` | – | 一段 JSON 字符串，放进 `data.payload` |
| `fail_times` | 0 | 同一 key 的前 N 次命中按失败响应，之后正常返回 |
| `fail_status` | 500 | 失败时的 HTTP 状态码 |
| `fail_code` | 同 fail_status | 失败时的业务码 |
| `fail_every` | 0 | 每 N 次命中失败一次，用来模拟周期性故障 |
| `fail_on` | – | 按命中序号指定哪几次失败，如 `fail_on=1,3`（第 1、3 次失败，其余成功）；与 `fail_times`/`fail_every` 同时出现时取并集 |
| `retry_after` | – | 失败时附带的 `Retry-After` 头（秒） |
| `size` | 0 | 在 `data.pad` 填充这么多字节，模拟大响应 |
| `fault` | – | 应用层坏响应，见下表 |
| `chunk_ms` | 50 | `fault=slow_body` 时每块的间隔 |
| `chunk_bytes` | 1024 | `fault=slow_body` 时每块的字节数；总块数 = ceil(body 长度 / chunk_bytes)，第一块与响应头一起发出 |
| `content_type` | json | 覆盖 Content-Type，可选 `text`、`html`、`octet` |
| `header` | – | `k:v` 形式的额外响应头，可以重复 |

`fault` 取值：

| fault | 行为 | 客户端预期 |
|---|---|---|
| `hang` | 收到请求后一直不回，最长 60s | 超时 |
| `close` | 读完请求直接关闭连接，不发响应 | 网络错误 |
| `half_body` | 发完响应头和一半 body 后关闭 | 网络错误或解析错误 |
| `slow_body` | body 分块慢慢吐，块间隔 `chunk_ms` | 成功但耗时长；可能触发接收超时 |
| `bad_json` | Content-Type 是 json，内容却非法 | 解析错误 |
| `empty` | 200 但 body 为空 | 看客户端怎么处理 |
| `html` | 200 加上 HTML 错误页，模拟网关拦截页 | 解析或业务错误 |
| `no_envelope` | 返回裸 JSON 数组或对象，没有信封 | 透传 |

## 4. 接口

### 4.1 回显与匿名

- **`ANY /t/mock/{*rest}`**：给 mock 类插件当 mock 服务用，匿名访问。行为同 echo，`data` 里额外带 `mock: true`，客户端由此判断请求确实被路由到了 mock。
  执行器把插件的 mock 基址设为 `<baseURL>/t/mock`。

- **`ANY /t/echo`**、**`ANY /t/public/{*rest}`**：匿名访问，按控制参数响应。`data` 里除了 `meta`，还有
  `request: {method, path, query, headers, body}`。headers 只回显 `authorization`、`content-type`、`x-*`；
  body 同时给原始字符串和解析后的 JSON（解析失败则为 null）。

### 4.2 鉴权

测试用的鉴权独立于线上的 `/auth/*`。用户随用随建，token 有效期精确到毫秒：

- **`POST /t/auth/login`**：请求体 `{username, password?, access_ttl_ms?=60000, refresh_ttl_ms?=600000, strict_rotation?=false}`。
  - 用户不存在时自动创建；`password = "wrong"` 时返回 401。
  - 返回 `{access_token, refresh_token, token_type:"Bearer", expires_in_ms}`。
- **`POST /t/auth/refresh`**：请求体 `{refresh_token}`，每次调用都轮换出一对新 token。
  - 用已经轮换过的旧 refresh_token 再来刷新，判为重用：返回 401，`reason = "reused"`，同时吊销整个 token 族，
    用来暴露"并发刷新了两次"的问题。
  - 过期返回 401，`reason = "expired"`；无效或不存在返回 401，`reason = "invalid"`。
  - 支持 `delay_ms` 和 `fail_times`，用来模拟慢刷新和刷新失败。
- **`POST /t/auth/logout`**：请求体 `{refresh_token}`，吊销它和对应的 access token。
- **`POST /t/auth/revoke`**：请求体 `{username, scope?}`，模拟服务器侧强制下线。
  - `scope = "all"`（默认）：该用户的所有 access token 和 refresh token 立即失效（`reason = "revoked"`）。这是真正的“踢下线”，客户端刷新也会失败，客户端错误类别应判 `expired`。
  - `scope = "access"`：只让 access token 失效，refresh token 仍然有效，客户端刷新后能恢复。用来模拟“服务器换了签名密钥”这类情况。
  - 返回 `data: {revoked:<数量>}`，数量为本次新吊销的未过期、未使用且尚未失效令牌数；重复吊销不重复计数。
- **`ANY /t/private/{*rest}`**：需要 `Authorization: Bearer <access>`。各种令牌问题的 code 一律是 `401`，
  具体原因放在 `data.reason` 里。HTTP 状态码由响应风格决定（§2.1）：http 风格是 401，business 风格是 200。

  | 情况 | `data.reason` | 客户端正确行为 |
  |---|---|---|
  | **无令牌**：没带 Authorization 头，或者头里是空 Bearer | `missing` | 不刷新；没有 refresh_token 时直接判未登录 |
  | **假令牌**：不是 JWT 格式、签名不对、被篡改过、`Bearer` 前缀写错 | `invalid` | 最多刷新一次，失败就判过期；**不能循环刷新** |
  | **过期令牌**：签名合法但已经超过 exp | `expired` | 刷新后重放 |
  | **旧令牌**：已被吊销，包括 logout、revoke，或者 refresh 之后被新令牌顶替（见下） | `revoked` | 刷新后重放；refresh 也失败就判过期 |

  - 客户端**不应该依赖 reason 才能正常工作**。很多真实后端只返回 401，不说原因，所以只看 code=401 也要能走通"刷新一次、
    失败判过期、绝不循环"这条通用路径。reason 只是给能读它的客户端做更精细的处理，比如无令牌时干脆不刷新。
  - "被顶替"：`/t/auth/login` 请求体里传 `strict_rotation: true` 时，每次 refresh 都会让之前发出的 access token 立即失效，
    再用就返回 `revoked`。默认不开启，此时旧 access 在过期之前仍然有效，这也是大多数后端的做法。客户端两种都要能处理。
  - 用户名以 `no_refresh_` 开头的测试用户，登录时只返回 access_token，不返回 refresh_token，用来模拟"拿不到 refresh 令牌"的情况。
  - `deny=1` 时返回 code `403`，`reason = "forbidden"`，模拟越权，HTTP 状态码同样由风格决定。
  - 其余行为和 echo 一致，`meta.auth` 是用户名。
- 失败码速查：
  - `401`：reason 取 missing、invalid、expired、revoked、reused、bad_credentials 之一；
  - `403`：reason 为 forbidden；
  - 其余注入的失败，code 等于注入的状态码，reason 为 `injected`。

### 4.3 并发编排

- **`ANY /t/barrier/{group}`**，查询参数：
  - `n`：这一组的请求数，上限 64；
  - `idx`：本请求在组内的序号，从 0 开始，必填；
  - `order`：放行顺序，取 `asc`、`desc`、`shuffle`、`given` 之一，`given` 时配合 `seq=2,0,1` 指定；
  - `seed`：shuffle 用的随机种子；
  - `gap_ms`：相邻两个放行之间的间隔，默认 30；
  - `fail`：哪些 idx 失败，例如 `1,3`，或者 `all`；
  - `wait_ms`：等不齐 n 个请求时的最长等待，默认 5000，超时后放行已到的请求，并在 `meta` 里标出 `partial: true`。
- 服务器把组内请求攒齐 n 个以后，严格按照 order 依次放行。**顺序、乱序、全部成功、全部失败、部分失败由服务器决定，
  不受网络抖动影响。**
- 响应 `data`：`{meta, idx, rank}`，`rank` 是实际放行名次。
- 同一 (run, group) 的一组放行完毕后自动重置，可以复用。

### 4.4 观测

- **`GET /t/runs/{run}`**：返回本轮的统计：
  ```json
  { "total": 12, "by_key": {"GET /t/echo": 3}, "by_path": {"/t/echo": 3},
    "peak_inflight": 4, "inflight": 0,
    "log": [ {"seq":1,"method":"GET","path":"/t/echo","key":"...","auth":null,
              "style":"http","status":200,"code":0,"reason":null,"start_ms":0,"end_ms":12,"aborted":false} ] }
  ```
  - `log` 最多保留 1000 条。`aborted: true` 表示客户端在服务器响应之前就断开了，这是验证"取消真的掐断了连接"的依据。
  - `peak_inflight` 是同时在途请求数的峰值，用来验证去重和并发数限制。
- **`DELETE /t/runs/{run}`**：清空这一轮的全部状态。
- **`GET /t/ping`**：一个很轻的连通性探针，返回 `{code:0}`，给 offline 插件探测网络用。

> **实现澄清（2026-10-27）**：探针、统计读取和轮次删除不计入业务 `total/by_key/by_path/inflight/log`，
> 避免观测行为干扰断言；统计与删除仍返回 `data.meta`，`hit` 沿用请求所属轮次的同键序号。
> `log.seq` 是到达序号，日志按完成顺序保存；
> 未形成响应就取消的请求记录 `status:0`。`aborted` 由 handler future 或响应体流被丢弃检测，
> 以服务器完成产出响应体为边界，不表示客户端应用已读取数据。服务端主动注入 close/half_body 不算客户端取消。
> 删除时已在途请求持有旧状态，不会向新建的同名轮次写入记录。

## 5. 服务器开关

- `TEST_MODE=1`：关闭限流和 CPU 熔断，避免并发用例被它们拦下。线上默认不开。
- `/t/*` 在线上也挂载，但有上限：barrier `n ≤ 64`，hang 最长 60s，`size ≤ 1MB`，每个轮次 log 最多 1000 条；同时存活的轮次最多 64 个（满了淘汰最久未访问的），每轮不同 key 最多 1024 个（超出后新 key 不再计数：hit 恒为 1、不进 by_key/by_path）。

> **实现澄清（2026-10-27）**：barrier 的 `rank` 从 0 开始；首个到达者锁定本代配置，组内配置不同、
> 重复或越界 idx 返回 400。已开始放行时到达的请求进入下一代；上一响应体结束或取消后，再等待 `gap_ms`
> 放行下一请求。取消的等待者不占实际放行名次。`gap_ms/wait_ms/chunk_ms` 也限制为 10000ms，
> 请求体限制 1MiB，`size` 超限截断为 1MiB；非法参数返回 400。HTTP 1xx 不是最终响应，`status/fail_status`
> 接受 200–599；HEAD、204、304 依 HTTP 规则不发响应体。特殊 fault 的裸体、坏 JSON、HTML 和空体有意不带 meta。
> hang 到达 60s 上限后主动关闭连接；自定义 header 保留 HTTP 帧边界字段（content-length、transfer-encoding、connection），这三项返回 400。

## 6. netem（网络条件代理）

`cargo run --release --bin netem -- --control 127.0.0.1:47090`。它是独立的可执行文件，**不打进线上 Docker 镜像**。

控制 API（JSON）：

| 方法 | 路径 | 说明 |
|---|---|---|
| `POST` | `/proxies` | 请求体 `{upstream:"127.0.0.1:47080", profile}`，返回 `{id, port}`，其中 port 是新分配的监听端口 |
| `PUT` | `/proxies/{id}` | 请求体 `{profile}`，立即切换，对已建立和新建的连接都生效 |
| `POST` | `/proxies/{id}/schedule` | 请求体 `[{at_ms, profile}]`，相对调用时刻按时间表切换，例如断 1500ms 后恢复 |
| `GET` | `/proxies/{id}/stats` | 返回 `{connections, active, bytes_up, bytes_down, resets, refused}` |
| `DELETE` | `/proxies/{id}` | 关闭代理及其上的全部连接 |
| `GET` | `/presets` | 内置预设列表 |

profile 可以直接写预设名的字符串，也可以写对象：

```json
{ "mode": "pass",                 // pass | refuse | blackhole | reset
  "latency_ms": 0, "jitter_ms": 0, // 每个方向、每个分块的延迟
  "down_kbps": 0, "up_kbps": 0,    // 0 表示不限速
  "first_byte_ms": 0,              // 首包额外延迟
  "reset_prob": 0.0,               // 每条连接以这个概率在中途发 RST
  "reset_after_bytes": null,       // 下行累计到这么多字节后发 RST
  "stall_prob": 0.0, "stall_ms": 0,// 分块级别的随机卡顿
  "seed": 1 }                      // 固定随机种子，保证可复现
```

- `refuse`：关闭监听，客户端得到 ECONNREFUSED；换成其它模式时恢复监听，端口不变。
- `blackhole`：接受连接但从不转发、从不回包，客户端得到超时。
- `reset`：接受连接后立即 RST，客户端得到 ECONNRESET。

内置预设（**场景里的 `networks` 字段只引用这些名字**）：

| 预设 | 含义 |
|---|---|
| `normal` | 直接转发 |
| `weak` | 延迟 300ms、抖动 200ms、下行 64kbps、上行 32kbps、首包延迟 200ms |
| `lossy` | 在 weak 基础上加 `reset_prob 0.15`、`stall_prob 0.1`、`stall_ms 800` |
| `offline` | refuse，表示网卡断开或服务不可达 |
| `blackhole` | 能连上但不回包，表示假连接或被防火墙丢包 |
| `flap` | 时间表：前 1500ms 为 offline，之后恢复 normal，用来验证断网恢复和重试 |
| `dns` | 不经过 netem；执行器把 baseURL 的 host 换成 `nonexistent.invalid`，得到 DNS 失败 |

> **实现澄清（2026-10-27）**：kbps 使用十进制千比特每秒（64kbps = 8000B/s），令牌桶按连接、按方向
> 独立，容量为一个 4096B 分块，没有初始突发额度。首包延迟按方向仅应用一次；抖动均匀取基础延迟的
> 正负 `jitter_ms` 范围并截断负值。固定 seed 在相同连接建立顺序及分块序列下可复现随机决策，
> TCP/操作系统调度和客户端观测耗时不保证逐次相同。
> `reset_prob` 每连接确定是否重置，并在接下来最多 4096B 下行内选定重置点；同时配置字节阈值时取最早条件。
> PUT 完整替换 profile 并取消旧时间表；schedule 替换上一张时间表。refuse 同时关闭已有连接，
> blackhole 保留已有连接的缓冲数据，恢复 pass 后可继续传输。内核拒绝连接时用户态无法逐次计数，
> 因此 stats.refused 记录进入停止监听状态的次数。`/presets` 返回名字到配置的映射；flap 为时间表，
> dns 为执行器域名说明，直接把 dns 用作代理 profile 返回 400。控制服务默认监听 127.0.0.1:47090，
> 代理监听随机的本机 IPv4 端口，上游使用明确的 IP:port。

## 7. 场景与预期

见 `scenario.schema.json`。要点：

- **能力声明**：场景用 `requires` 声明需要哪些能力，名字与语言无关，取 `auth`、`cache`、`retry`、`share`、`cancel`、
  `loading`、`mock`、`normalize`、`notify`、`repath`、`key`、`filter`、`envs`、`breaker`、`offline`、`timeout`、`log`。
  客户端缺少某项能力时跳过该场景，并在报告里写 `skipped: missing <cap>`，不算失败。
- **`filter` 能力的中立语义**：`client.plugins.filter` 只表示"装配了过滤能力"，**不会对任何请求自动生效**；
  请求必须显式写 `options.filter: true` 才开启。开启后，客户端在发送前剔除查询参数（`query`）和请求体（`body`，
  仅当其为 JSON 对象）**顶层**的"空值"字段：`null`、`undefined`、`NaN`、空白字符串（仅含空白字符，含 `""`）；
  `0`、`false` 等有效值必须保留，嵌套对象不递归。未写或写 `false` 时请求原样发送。
  断言落点是服务端实际收到的参数（`/t/echo` 回显的 `request.query`、`request.body.json`）。
  `ignoreKeys`、`ignoreValues`、自定义 predicate 属于各库的私有扩展，**不在中立表达范围内**，场景不得依赖。
  注意：部分 HTTP 库序列化查询串时本来就会丢掉 `null`/`undefined`，所以"剔除 null"只在请求体上才能可靠断言，
  查询参数上用空白字符串断言。
- **插件配置**：`client.plugins` 写中立配置，各执行器自己映射成本库的参数。映射不了的字段要显式报错，不能静默忽略。
- **响应风格维度**：每个场景默认在 `http` 和 `business` 两种风格下各跑一遍（§2.1）。场景可以用 `styles` 字段收窄范围。
- **错误分类**：预期里的错误统一用下面这些取值，各执行器负责把本库的错误归到这些类别：
  - `status:<n>`：**推荐写法，与风格无关**。表示逻辑状态是 n：http 风格下按 `http:<n>` 匹配，business 风格下按 `business:<n>` 匹配；
  - `http:<status>`：HTTP 状态码错误，只在需要专门断言 http 风格时使用；
  - `business:<code>`：业务码错误，包括 HTTP 200 加非 0 code 的情况；
  - `timeout`：超时；
  - `canceled`：主动取消；
  - `network`：refused、reset、dns、连接中断；
  - `parse`：响应解析失败；
  - `breaker_open`：熔断器打开；
  - `offline`：离线插件判定无网；
  - `denied`：越权；
  - `expired`：会话过期，刷新失败或 refresh 也已失效；
  - `unauthenticated`：根本没登录，无令牌且没有 refresh 令牌，不应发起刷新。
- **客户端事件**：执行器统一采集这些事件供断言用：
  - `loading`：loading 显示和隐藏的序列，例如 `[true,false]`；
  - `notify`：提示触发的次数及每次的消息；
  - `refresh`：刷新 token 的调用次数；
  - `expired`：会话过期回调的次数；
  - `denied`：越权回调的次数；
  - `pending_after`：场景结束后还没结束的请求数，必须为 0。

---

## 附录 A：响应体示例

所有示例都是完整的 HTTP 响应体。`meta` 字段的结构见 §2，这里不再展开。

### A.1 回显与匿名

`ANY /t/echo`、`ANY /t/public/*`，成功（HTTP 200）：

```json
{ "code": 0, "message": "ok",
  "data": {
    "meta": { "run": "r1", "key": "POST /t/echo", "hit": 1, "path": "/t/echo",
              "method": "POST", "auth": null, "ts": 1730000000000 },
    "request": {
      "method": "POST", "path": "/t/echo", "query": { "a": "1" },
      "headers": { "content-type": "application/json", "x-trace": "1" },
      "body": { "raw": "{\"x\":1}", "json": { "x": 1 } } },
    "payload": null,
    "pad": "" } }
```

- 请求带了 `data=` 参数时，`payload` 是解析后的 JSON。
- 请求带了 `size=` 参数时，`pad` 是相应长度的填充字符串。

`GET /t/ping`：

```json
{ "code": 0, "data": { "pong": true }, "message": "ok" }
```

### A.2 鉴权

`POST /t/auth/login` 和 `POST /t/auth/refresh` 成功时：

```json
{ "code": 0, "message": "ok",
  "data": { "access_token": "eyJ...", "refresh_token": "8c1f...", "token_type": "Bearer", "expires_in_ms": 60000 } }
```

`no_refresh_*` 用户登录时，`data` 里没有 `refresh_token` 字段。

`POST /t/auth/logout` 的 `data` 是 `{ "logged_out": true }`，`POST /t/auth/revoke` 的 `data` 是 `{ "revoked": 3 }`（吊销的令牌数）。

`ANY /t/private/*` 成功时和 A.1 相同，只是 `meta.auth` 为用户名。

各类失败。HTTP 状态码列按 http 风格给出，business 风格下一律是 200，响应体不变：

| 情况 | HTTP（http 风格） | code | `data.reason` | message |
|---|---|---|---|---|
| 无令牌 | 401 | 401 | `missing` | `missing token` |
| 登录密码错误 | 401 | 401 | `bad_credentials` | `invalid username or password` |
| 假令牌 | 401 | 401 | `invalid` | `invalid token` |
| 过期令牌 | 401 | 401 | `expired` | `token expired` |
| 旧令牌 | 401 | 401 | `revoked` | `token revoked` |
| refresh 无效 | 401 | 401 | `invalid` | `invalid refresh token` |
| refresh 过期 | 401 | 401 | `expired` | `refresh token expired` |
| refresh 重用 | 401 | 401 | `reused` | `refresh token reused` |
| refresh 已吊销 | 401 | 401 | `revoked` | `token revoked` |
| 越权 | 403 | 403 | `forbidden` | `forbidden` |

两种风格下的同一个响应，以过期令牌为例：

```text
# http 风格
HTTP/1.1 401 Unauthorized
{ "code": 401, "data": { "reason": "expired" }, "message": "token expired" }

# business 风格
HTTP/1.1 200 OK
{ "code": 401, "data": { "reason": "expired" }, "message": "token expired" }
```

### A.3 并发编排

`ANY /t/barrier/{group}` 放行成功时：

```json
{ "code": 0, "message": "ok",
  "data": { "meta": { "run": "r1", "key": "GET /t/barrier/g1", "hit": 3, "partial": false }, "idx": 2, "rank": 0 } }
```

`fail` 命中的 idx 按 `fail_status` / `fail_code` 返回。默认情况下，http 风格是 HTTP 500，business 风格是 HTTP 200，响应体都是
`{ "code": 500, "data": { "reason": "injected", "idx": 1, "rank": 3 }, "message": "barrier fail" }`。

### A.4 控制参数产生的失败

- `fail_times` / `fail_every` 命中时，按 `fail_status` / `fail_code` 返回。默认情况下，http 风格是 HTTP 500，business 风格是 HTTP 200，响应体是
  `{ "code": 500, "data": { "reason": "injected" }, "message": "injected failure" }`。设置了 `retry_after` 时，两种风格都会带 `Retry-After` 头。
- `status` / `code` 直接指定的响应：
  - `status=503` → http 风格 HTTP 503，business 风格 HTTP 200，code 都是 503；
  - `code=1001`（不传 status）→ 两种风格都是 HTTP 200，code 为 1001，表示纯业务失败；
  - 失败响应的 `data` 都是 `{ "reason": "injected" }`；message 取 `message` 参数，默认 `ok`。

### A.5 观测

`GET /t/runs/{run}`：

```json
{ "code": 0, "message": "ok", "data": {
    "total": 12, "inflight": 0, "peak_inflight": 4,
    "by_key":  { "GET /t/echo": 3 },
    "by_path": { "/t/echo": 3, "/t/auth/refresh": 1 },
    "log": [ { "seq": 1, "method": "GET", "path": "/t/private/a", "key": "GET /t/private/a",
               "auth": "u1", "style": "http", "status": 401, "code": 401, "reason": "expired",
               "start_ms": 0, "end_ms": 12, "aborted": false } ] } }
```

`DELETE /t/runs/{run}` 返回 `{ "code": 0, "data": { "cleared": true }, "message": "ok" }`。

### A.6 netem 控制 API

netem 的控制 API 不用信封，直接返回 JSON：

| 接口 | 响应 |
|---|---|
| `POST /proxies` | `{ "id": "p1", "port": 41234 }` |
| `PUT /proxies/{id}`、`POST /proxies/{id}/schedule`、`DELETE /proxies/{id}` | `{ "ok": true }` |
| `GET /proxies/{id}/stats` | `{ "connections": 5, "active": 1, "bytes_up": 1200, "bytes_down": 64000, "resets": 0, "refused": 2 }` |
| `GET /presets` | `{ "normal": {...profile}, "weak": {...}, ... }` |
| 请求参数错误 | HTTP 400 加上 `{ "error": "..." }` |

---

## 附录 B：语义澄清（2026-10-27，D11）

1. **`status` 与 `code` 同时出现**，例如 `status=500&code=5001`：
   - http 风格：HTTP 500，`code = 5001`。客户端优先按 HTTP 状态判定，应归为 `status:500`，也就是 `http:500`。
   - business 风格：HTTP 200，`code = 5001`，应归为 `business:5001`。
   - 场景想断言与风格无关的结果时，不要同时给 status 和 code。
2. **`login`、`logout`、`revoke`、`server_reset` 这几个步骤走旁路**：执行器直接连服务器，不经过 netem，也不经过被测客户端的插件链，只把拿到的 token 写进客户端的 token 存储。
   它们只是准备环境，不是被测对象。因此含 login 的场景同样可以跑 offline、dns、blackhole 网络，被测请求照常经过 netem。
3. **`set_token` 里 `previous` 的含义**：指客户端持有的 access 或 refresh 在最近一次变化之前的值。“变化”包括 login、刷新成功、`set_token`、`expire_local`。
   执行器每次替换令牌时都要记下旧值。
4. **运行中切换响应风格**：新增 step `style: "http" | "business"`，从下一个请求开始生效，用来模拟后端灰度切换风格。
5. **barrier 失败**：按 `fail_status`/`fail_code` 输出（附录 A.3），逻辑状态默认是 500，同样受响应风格控制。
