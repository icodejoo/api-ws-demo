# api-ws-demo

一个用 Rust（axum）实现的测试服务器，同时提供 REST API、原始 WebSocket、STOMP 1.2 over WebSocket
三种通信方式，专门用来给各种客户端（浏览器、移动端、后端服务等）做联调/兼容性测试。由 GitHub Actions
构建成 Docker 镜像并推送到 GHCR，部署在 Render.com 免费 Web Service（"Existing Image" 部署方式——
Render 只拉取镜像直接运行，完全不在 Render 侧编译）。

> 给用 Claude Code 参与本项目开发的人：项目背景/架构/硬性约束见 [`CLAUDE.md`](./CLAUDE.md)；如果改了
> `assets/compressed_sample.json`，记得同步重新生成另外 5 个压缩衍生文件，具体步骤见
> `.claude/skills/regenerate-compressed-assets/SKILL.md`。

## 统一响应格式

除了 `/api/echo` 和几个静态压缩测试接口（返回原始字节）之外，所有 REST 接口的 JSON 响应都遵循同一个
封装格式：

```json
{ "code": 0, "data": { ... }, "message": "ok" }
```

- `code`：业务码，`0` 表示成功，非 `0` 表示具体错误（同时 HTTP 状态码也会是对应的 4xx/5xx）。
- `data`：真正的业务数据，失败时为 `null`。
- `message`：人类可读的说明文字。

CORS 是完全放开的（`Access-Control-Allow-Origin: *` 等），包括限流/CPU 熔断触发的错误响应也会带上
这些头——因为这是一个纯测试服务器，不涉及需要凭证/Cookie 的会话，放开跨域没有实际风险。

## REST 接口

### 基础

- **`GET /`** —— 首页，返回当前所有可用接口列表（JSON），方便直接打开浏览器看一眼服务是否正常、有哪些接口。
- **`GET /health`** —— 健康检查，返回 `{status, version, uptime_seconds}`。Render 的健康检查也是打这个接口。
- **`GET /api/info`** —— 和 `/health` 类似，返回服务名/版本号/运行时长，纯粹的信息查询接口。
- **`GET /api/stats`** —— 查看当前服务器资源占用：
  - `cpu_percent`：跟 CPU 熔断用的是同一个采样值。
  - `memory`：`total_kb`/`available_kb`/`used_kb` **优先读容器的 cgroup 内存限额**（`memory.source`
    会标 `"cgroup_v2"` 或 `"cgroup_v1"`），这才是 Render 实际给这个容器分配的配额；只有在没有设置
    cgroup 限额时（比如本地非容器化的 Linux）才会退化成读 `/proc/meminfo` 的整机内存（`source` 会标
    `"system"`）——这种情况下数字是宿主机级别的，不代表容器配额，仅供参考。`process_rss_kb` 是本进程
    自己占用的常驻内存，这个不管哪种情况都是准的。另外还有 `used_percent`（保留一位小数，比如 `0.2`、
    `60.5`、`90.1`，避免占用很少时被整数百分比四舍五入成 `0` 而看不出实际情况）。
  - `disk`：挂载点 `/` 的 `total_bytes`/`available_bytes`/`used_bytes`/`used_percent`，通过 `statvfs`
    系统调用读取（这个目前还是宿主机/所在卷的视角，不是 cgroup 级别的配额）。
  这几项数据全部来自 Linux 的 `/proc`、`/sys/fs/cgroup` 伪文件系统和 `statvfs` 系统调用，**只在 Linux
  容器里有效**——本地 Windows/macOS 开发时这几个字段会是 `null`（优雅降级，不会报错）。
- **`POST /api/echo`** —— 原样把请求体和 `Content-Type` 回显给客户端（不套用统一响应格式，因为它的
  用途就是测试"发什么就收到什么"，包括非 JSON 的二进制内容，套上 JSON 封装会破坏这个语义）。

### 万能接口 `/api/mock`（重点说明）

**用途**：这是一个完全由调用方通过参数"遥控"服务器该怎么响应的接口——不需要改代码、不需要重新部署，
直接在请求里指定"我要一个什么样的响应"，服务器就照办。典型用途：

- 测试客户端的**超时/loading 状态处理**：用 `delay_ms` 让服务器故意卡住几秒再响应。
- 测试客户端对**各种 HTTP 状态码**的处理逻辑（404、500、503……），不需要真的触发服务器错误。
- 测试客户端对**业务层错误码**（`code` 字段非 0）的处理，配合 HTTP 状态码一起模拟"接口返回业务失败"的场景。
- 让前端/移动端在没有真实后端接口时，先拿这个接口**造数据**联调。

**请求方式**：`GET` 和 `POST` 都支持，走同一个逻辑，参数含义完全一致：

| 参数 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `delay_ms` | 整数 | `0` | 服务器收到请求后先等待这么多毫秒再响应，模拟慢接口。**会被强制限制在 `[0, 10000]` 毫秒内**（超过 10 秒的值会被截断为 10 秒，而不是报错）——这是为了防止免费实例上出现请求无限挂起占用连接资源。 |
| `status` | 整数 | `200` | 响应用的 HTTP 状态码，必须是合法的 HTTP 状态码（100–599），否则返回 `400` 并提示非法状态码。 |
| `code` | 整数 | `0` | 响应体里 `code` 字段的值，用来模拟业务层的成功/失败码，和 HTTP 状态码是两个独立的维度（可以 HTTP 200 但业务 code 非 0，模拟"请求成功但业务失败"）。 |
| `message` | 字符串 | `"mock response"` | 响应体里的 `message` 字段。 |
| `data` | 任意 JSON（仅 POST 支持嵌套结构）/字符串（GET 只能传扁平字符串） | `null` | 响应体里的 `data` 字段。GET 是走 query string，只能传单一字符串；POST 走 JSON body，可以传任意嵌套的 JSON 结构（对象、数组都行）。 |

**用法示例**：

```bash
# GET：500ms 延迟 + 201 状态码 + 自定义业务码和消息
curl "http://localhost:8080/api/mock?delay_ms=500&status=201&code=7&message=hello"
# -> HTTP 201, {"code":7,"data":null,"message":"hello"}

# POST：模拟一个"业务失败但HTTP层是200"的场景，且 data 是嵌套对象
curl -X POST -H "Content-Type: application/json" \
  -d '{"code":42,"message":"库存不足","data":{"itemId":123,"remaining":0}}' \
  http://localhost:8080/api/mock
# -> HTTP 200, {"code":42,"data":{"itemId":123,"remaining":0},"message":"库存不足"}

# 故意传一个非法状态码
curl "http://localhost:8080/api/mock?status=9999"
# -> HTTP 400, {"code":400,"data":null,"message":"invalid status code: 9999"}
```

### 静态压缩/编码测试接口

这一组接口的作用是让客户端测试自己**对压缩内容/二进制序列化格式的解析能力**——每个接口背后的数据都是
**服务启动前就已经压缩/编码好、打包进二进制文件里的固定测试数据**（通过 Rust 的 `include_bytes!`
在编译期嵌入，见 `assets/` 目录），服务器运行时**不会做任何实时压缩/编码计算**，无论多少客户端同时
请求都不会消耗额外 CPU——这一点对 CPU 很弱的免费实例尤其重要。

| 接口 | Content-Type | Content-Encoding | 说明 |
|---|---|---|---|
| `GET /api/compressed` | `application/json` | `gzip` | JSON 数据，gzip 压缩。大多数 HTTP 客户端（包括 `curl --compressed`）会自动识别 `Content-Encoding: gzip` 并透明解压，这个接口主要测的是"客户端有没有正确声明/处理 gzip"。 |
| `GET /api/compressed-zstd` | `application/json` | `zstd` | JSON 数据，zstd 压缩。zstd 的客户端原生支持远没有 gzip 普及（尤其浏览器），这个接口专门用来测试客户端是否具备 zstd 解压能力。 |
| `GET /api/compressed-mp` | `application/msgpack` | （无） | 未压缩的 MessagePack 二进制序列化数据，测试客户端解析 MessagePack 格式的能力（和 JSON 相比更紧凑）。 |
| `GET /api/compressed-mp-gzip` | `application/msgpack` | `gzip` | MessagePack + gzip 双重处理，测试"先解压再解析二进制格式"的组合能力。 |
| `GET /api/compressed-mp-zstd` | `application/msgpack` | `zstd` | MessagePack + zstd。 |

用法示例：

```bash
curl --compressed http://localhost:8080/api/compressed        # curl 自动解压并打印可读 JSON
curl -s http://localhost:8080/api/compressed | gunzip          # 手动解压，效果一样
curl -s http://localhost:8080/api/compressed-mp | xxd | head    # 看原始 MessagePack 字节
```

STOMP 协议下也有完全对应的 5 个 topic（见下文 STOMP 部分），行为和数据完全一致，只是传输方式换成了
WebSocket 推送而不是一次性 HTTP 响应。

### 登录认证 `/auth/*` 和 `/api/me`

一套极简的 JWT 登录体系，**用户数据全部存在内存里**（服务重启/重新部署就会清空——这是刻意的设计，
因为这只是个测试服务器，不需要真正持久化用户数据）。

- **`POST /auth/register`** —— 请求体 `{"username", "password"}`，注册新用户。用户名重复会返回
  `409 Conflict`。密码用 HMAC-SHA256 加盐哈希（刻意不用 argon2/bcrypt 这类"故意很慢"的算法——那是
  为了抗暴力破解设计的，但会实打实消耗 CPU，这里是测试数据，没有真实密码安全的顾虑，换来的是免费实例
  CPU 资源的节省）。
- **`POST /auth/login`** —— 请求体 `{"username", "password"}`，验证成功后返回：
  ```json
  {"code":0,"data":{"access_token":"...", "refresh_token":"...", "token_type":"Bearer", "expires_in":900},"message":"ok"}
  ```
  `access_token` 是短期有效（15 分钟）的 JWT，用于访问需要鉴权的接口；`refresh_token` 是长期有效
  （7 天）的**不透明字符串**（服务器端可撤销的 UUID，不是 JWT），用来在 access_token 过期后换新。
- **`POST /auth/refresh`** —— 请求体 `{"refresh_token"}`，用旧的 refresh_token 换一对新的
  access_token + refresh_token（**旧的 refresh_token 会立刻失效**，即"轮转"机制，防止同一个
  refresh_token 被重复使用）。
- **`POST /auth/logout`** —— 请求体 `{"refresh_token"}`，主动吊销这个 refresh_token，之后再拿它去
  刷新会被拒绝。
- **`GET /api/me`** —— 需要在请求头带 `Authorization: Bearer <access_token>`，返回当前登录用户名。
  没带 token 或 token 无效/过期都返回 `401`。

## WebSocket 接口

- **`GET /ws`** —— 最基础的原始 WebSocket，服务器把收到的每一条文本/二进制帧原样发回去（纯 echo），
  不需要任何鉴权，直连即可，用来测试最基本的 WebSocket 连通性和帧收发。
- **`GET /ws/secure`** —— 行为和 `/ws` 完全一样（也是 echo），唯一区别是连接时必须在 URL 上带
  `?token=<access_token>` 查询参数，服务器会在升级连接前校验这个 token，无效/缺失直接拒绝握手
  （不会建立连接）。用来测试"带鉴权的 WebSocket 连接"场景（浏览器原生 WebSocket API 不支持自定义
  请求头，所以约定俗成用 query 参数传 token）。

## STOMP 接口 `/stomp`

一个建在 WebSocket 之上的、极简的内存版 STOMP 1.2 消息代理（broker），支持 CONNECT、SUBSCRIBE、
UNSUBSCRIBE、SEND、ACK、NACK、DISCONNECT、ERROR 这几种帧。没有实现 SockJS 兼容层。握手时正确
协商 `v12.stomp`/`v11.stomp`/`v10.stomp` WebSocket 子协议——标准 STOMP 客户端库（比如
`@stomp/stompjs`）连接时会请求这几个子协议之一，按 WebSocket 规范服务端不确认的话客户端会
直接中止连接，裸手写的 WebSocket 测试脚本因为不主动请求子协议，不会碰到这个问题。

### 基本用法

1. 连接 WebSocket 到 `/stomp`。
2. 发送 `CONNECT` 帧（必须是第一条消息），服务器回复 `CONNECTED`。
3. 发送 `SUBSCRIBE` 帧订阅某个 destination（比如 `/topic/room1`），之后所有发到这个 destination 的
   消息都会被广播给你；**订阅成功 3 秒后，服务器一定会主动推一条消息给你**（不管这期间有没有人往这个
   destination 发送过东西）——细节见下面"订阅后自动推送"。
4. 发送 `SEND` 帧往某个 destination 发消息，服务器会把这条消息包装成 `MESSAGE` 帧广播给所有订阅了
   这个 destination 的连接（包括自己，如果自己也订阅了的话）。
5. 发送 `UNSUBSCRIBE` 取消订阅；发送 `DISCONNECT` 主动断开。

这是一个纯内存的广播路由，**不做消息持久化**，服务重启后所有订阅关系都会清空——纯粹用来测试客户端
STOMP 协议实现是否正确（连接握手、订阅、收发消息、断开的完整流程）。

### 心跳（heart-beat）

真正实现了 STOMP 1.2 的双向心跳协商，不是摆设：

- `CONNECT` 帧可以带 `heart-beat:<cx>,<cy>` 头（`cx` = 你能保证发送心跳的最小间隔毫秒数，`cy` = 你希望
  收到心跳的间隔毫秒数；`0` 表示对应方向不需要/不提供）。不带这个头，等价于 `0,0`（双向都不启用）。
- 服务器自己的心跳能力是 `60000,60000`（1 分钟）。最终协商结果是双方数值中**较大**的那个（取更保守/更慢
  的一方，避免一方来不及跟上）；如果任意一方对某个方向传了 `0`，那个方向就直接禁用。协商结果会写在
  `CONNECTED` 帧的 `heart-beat` 头里，返回的是**协商后的实际值**，不是服务器的默认值。
- 心跳字节本身是裸的 `\n`（不是完整的 STOMP 帧，没有 command/header/body）。
- 服务器如果超过"协商的接收间隔 × 3"都没收到任何数据（心跳或正常帧都算），会判定连接已死，发一条
  `ERROR` 帧后主动断开。按 1 分钟心跳算，这个超时（3 分钟）和下面的连接最长存活时间（3 分钟）基本
  重合，两道机制谁先触发都行，不再像更短心跳间隔那样有明显的先后错开。

### 最长连接时间

每个 STOMP 连接**硬性最长存活 3 分钟**，不管连接是否还在正常心跳/收发消息，到点就会收到
`ERROR message:connection time limit exceeded, please reconnect` 然后被强制断开——避免免费实例上
出现大量长连接占用资源。

### ACK / NACK

`SUBSCRIBE` 帧可以带 `ack:auto|client|client-individual` 头（默认 `auto`，也就是现在这样不需要确认）。
用 `client`/`client-individual` 模式订阅后，你收到的每条 `MESSAGE` 帧都会带一个 `ack` 头（一个不透明的
id）。用 `ACK` 或 `NACK` 帧、带上这个 id 作为 `id` 头发回去，服务器会回一条 `RECEIPT` 帧，body 是
`{"status":"ok"}`，确认收到了你的 ACK/NACK。

**注意**：这里的 ACK/NACK 只是协议层面的"确认收到"记账，**NACK 不会触发消息重投递**——broker 本身是
纯广播、没有消息队列/持久化，NACK 之后消息不会重新发给你，只是告诉服务器"我处理失败了"这件事被记录了。
用同一个 id 重复 ACK/NACK（已经确认过、或者压根不认识的 id）会收到 `ERROR` 帧，但连接不会断开。

### 订阅后自动推送

`SUBSCRIBE` 成功后，**3 秒后服务器一定会主动推一条消息给你**，不管这期间有没有人往这个 destination
发过消息：

- 如果是下面表格里那 5 个静态压缩 topic —— 直接推对应的静态数据（跟主动 SEND 触发的效果一样），不需要
  任何人先 SEND 过。
- 如果是其他任意/自定义 destination —— 服务器会记住"最后一次有人 SEND 到这个 destination 的内容"：
  - 如果之前有人发过（不管是不是你自己发的），推 `{"response": <最后一条内容>}`。
  - 如果这个 destination 从来没人发过，推 `{"response": "ready"}`。

也就是说，普通 destination 的 SEND 广播语义也变了：现在 SEND 到非静态 topic，广播出去的内容会被包一层
JSON——发 `"hello"` 过去，所有订阅者收到的是 `{"response":"hello"}`，同时这条内容也会被缓存下来，后面
新订阅这个 destination 的人 3 秒后收到的就是这条缓存内容。

### 鉴权规则（按 destination 前缀区分）

- **`/topic/public/*`** —— 完全开放，任何人（不带 Authorization 都行）都可以订阅/发送。
- **`/topic/secure/*`** —— 必须在 `CONNECT` 帧里带 `Authorization: Bearer <access_token>` 头
  （`access_token` 就是 `/auth/login` 拿到的那个）才能订阅/发送到这类 destination。注意区别：
  - **完全不带 Authorization 头** —— 连接正常建立，但只能用 `/topic/public/*`，碰 `/topic/secure/*`
    会收到一条 `ERROR` 帧（但连接不会被断开，还可以继续用公共 topic）。
  - **带了但 token 无效/过期** —— `CONNECT` 直接被拒绝，连接不会建立（因为一个"看起来带了 token
    但实际无效"的情况，比"完全没带"更可能是客户端的 bug，直接报错比静默降级成匿名更安全）。

### 五个静态压缩测试 topic

和上面 HTTP 版 `/api/compressed*` 完全对应的一组 topic。SEND 任何内容到这些 topic，服务器都会无视
你发的内容，直接把对应的静态预压缩数据作为 `MESSAGE` 广播给所有订阅者；**SUBSCRIBE 成功后 3 秒也会
自动收到同样的数据，不需要任何人先 SEND**（同样是纯静态数据，零运行时压缩开销）：

| Topic | content-type 头 | content-encoding 头 |
|---|---|---|
| `/topic/compressed` | `application/json` | `gzip` |
| `/topic/compressed-zstd` | `application/json` | `zstd` |
| `/topic/compressed-mp` | `application/msgpack` | （无） |
| `/topic/compressed-mp-gzip` | `application/msgpack` | `gzip` |
| `/topic/compressed-mp-zstd` | `application/msgpack` | `zstd` |

这两个字段作为 STOMP 帧的自定义 header 附在 `MESSAGE` 帧上（不是 HTTP 协议层面的头，是 STOMP 帧里的
文本 header），客户端收到消息后可以读这两个 header 来决定怎么解码 body。这几个 topic 完全开放，不需要
鉴权。

## 限流与 CPU 熔断

- **按 IP 限流**（基于 `tower_governor`）：识别 `X-Forwarded-For`/`X-Real-Ip` 头来判断真实客户端 IP
  （因为 Render 在前面加了一层反向代理，直接看 TCP 连接的对端地址只会看到 Render 自己的负载均衡器）。
  超过限制返回 `429 Too Many Requests`。环境变量可调：`RATE_LIMIT_PER_SECOND`（默认 5）、
  `RATE_LIMIT_BURST`（默认 10，允许短时间内的突发请求）。
- **CPU 熔断**：后台每秒读一次 `/proc/stat` 算出 CPU 使用率（仅 Linux 有效，本地 Windows/macOS
  开发时永远读到 0%，熔断不会触发），一旦达到阈值（环境变量 `CPU_BREAKER_THRESHOLD_PCT`，默认
  `90`），**所有新请求会立刻收到 `503`**（连限流那一层都不会走到，是最外层最早拦截的一道防线），
  等 CPU 降下来之后自动恢复正常。
- `JWT_SECRET`：JWT 签名密钥。不设置的话服务启动时会随机生成一个（反正用户数据本来就是内存态的，
  重启就清空，密钥跟着一起换掉没有实际影响）。

## 本地开发

```powershell
cargo test
$env:PORT = "8080"
$env:RUST_LOG = "info,api_ws_demo=debug"
cargo run
```

```powershell
curl http://localhost:8080/health
curl http://localhost:8080/api/info
curl -X POST -H "Content-Type: application/json" -d '{"hello":"world"}' http://localhost:8080/api/echo
```

原始 WebSocket（用 `websocat` 或 `wscat`）：

```powershell
websocat ws://localhost:8080/ws
```

STOMP（Git-Bash/WSL 下用 `printf`，因为需要发送 NUL 帧结束符）：

```bash
printf 'CONNECT\naccept-version:1.2\nhost:localhost\n\n\0' | websocat ws://localhost:8080/stomp
```

## Docker

```powershell
docker build -t api-ws-demo:local .
docker run --rm -p 8080:8080 -e PORT=8080 api-ws-demo:local
```

## 部署到 Render.com

1. 推送到 `main` 分支 —— GitHub Actions 会构建镜像并推送到 `ghcr.io/icodejoo/api-ws-demo`，同时打上
   `latest` 和当次提交短 SHA 两个 tag。
2. **一次性操作**：到 GHCR 镜像包页面把可见性设为 **Public**（Settings → Danger Zone）。用默认
   `GITHUB_TOKEN` 推送的镜像包无论仓库本身是否公开，首次创建时都是私有的，Actions 没有办法直接
   把它设为公开。
3. 在 Render 控制台里，用本仓库的 `render.yaml` Blueprint 创建 Web Service（或者手动创建：
   New → Web Service → 来源选 "Existing Image" → 填 `ghcr.io/icodejoo/api-ws-demo:latest`，
   套餐选 Free）。
4. 复制该服务的 Deploy Hook URL（Settings → Deploy Hook），添加为本仓库 GitHub Actions 的
   `RENDER_DEPLOY_HOOK_URL` secret。因为 Render 不会主动轮询镜像仓库有没有新 tag，所以每次
   构建完成后 workflow 会手动调用这个 hook 来触发一次重新部署。
