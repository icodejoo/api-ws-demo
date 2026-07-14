# api-ws-demo

一个用 Rust（axum）实现的测试服务器，同时提供 REST API、原始 WebSocket、STOMP 1.2 over WebSocket
三种通信方式，专门用来给各种客户端（浏览器、移动端、后端服务等）做联调/兼容性测试。由 GitHub Actions
构建成 Docker 镜像并推送到 GHCR，部署在 Render.com 免费 Web Service（"Existing Image" 部署方式——
Render 只拉取镜像直接运行，完全不在 Render 侧编译）。

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
- **`GET /api/stats`** —— 查看当前服务器资源占用：`cpu_percent`（跟 CPU 熔断用的是同一个采样值）、
  `memory`（`total_kb`/`available_kb`/`used_kb` 是整机内存，`process_rss_kb` 是本进程自己占用的常驻内存）、
  `disk`（挂载点 `/` 的 `total_bytes`/`available_bytes`/`used_bytes`，通过 `statvfs` 系统调用读取）。
  这几项数据全部来自 Linux 的 `/proc` 伪文件系统和 `statvfs` 系统调用，**只在 Linux 容器里有效**——本地
  Windows/macOS 开发时这几个字段会是 `null`（优雅降级，不会报错）。
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
UNSUBSCRIBE、SEND、DISCONNECT、ERROR 这几种基本帧。没有实现 SockJS 兼容层。

### 基本用法

1. 连接 WebSocket 到 `/stomp`。
2. 发送 `CONNECT` 帧（必须是第一条消息），服务器回复 `CONNECTED`。
3. 发送 `SUBSCRIBE` 帧订阅某个 destination（比如 `/topic/room1`），之后所有发到这个 destination 的
   消息都会被广播给你。
4. 发送 `SEND` 帧往某个 destination 发消息，服务器会把这条消息包装成 `MESSAGE` 帧广播给所有订阅了
   这个 destination 的连接（包括自己，如果自己也订阅了的话）。
5. 发送 `UNSUBSCRIBE` 取消订阅；发送 `DISCONNECT` 主动断开。

这是一个纯内存的广播路由，**不做消息持久化**，服务重启后所有订阅关系都会清空——纯粹用来测试客户端
STOMP 协议实现是否正确（连接握手、订阅、收发消息、断开的完整流程）。

### 鉴权规则（按 destination 前缀区分）

- **`/topic/public/*`** —— 完全开放，任何人（不带 Authorization 都行）都可以订阅/发送。
- **`/topic/secure/*`** —— 必须在 `CONNECT` 帧里带 `Authorization: Bearer <access_token>` 头
  （`access_token` 就是 `/auth/login` 拿到的那个）才能订阅/发送到这类 destination。注意区别：
  - **完全不带 Authorization 头** —— 连接正常建立，但只能用 `/topic/public/*`，碰 `/topic/secure/*`
    会收到一条 `ERROR` 帧（但连接不会被断开，还可以继续用公共 topic）。
  - **带了但 token 无效/过期** —— `CONNECT` 直接被拒绝，连接不会建立（因为一个"看起来带了 token
    但实际无效"的情况，比"完全没带"更可能是客户端的 bug，直接报错比静默降级成匿名更安全）。

### 五个静态压缩测试 topic

和上面 HTTP 版 `/api/compressed*` 完全对应的一组 topic，SEND 任何内容到这些 topic，服务器都会无视
你发的内容，直接把对应的静态预压缩数据作为 `MESSAGE` 广播给所有订阅者（同样是纯静态数据，零运行时
压缩开销）：

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
