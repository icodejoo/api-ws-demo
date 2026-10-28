# testkit 设计决策记录

按时间顺序记录 testkit 的设计决策，方便日后追溯。每条写清背景、选项、结论、理由，以及谁拍的板。
推翻旧决策时，追加一条新的记录并引用旧条目；被明确废弃的业务码从旧条目清理，以当前契约为准。

---

## D1 · 2026-10-27 · 为什么要做 testkit

- **背景**：Codex 对 axp 做全量 review，报出 30 条问题，终审确认 29 条。其中大量是插件**组合**、**并发**、
  **网络异常**下的问题，例如 auth 重放和 cancel、retry 和 loading、cache 和 share 之间的冲突。
  现有的单测在内存里 mock adapter，覆盖不到真实的连接行为；浏览器 e2e 依赖线上服务，命中计数是写死的常量，断言实际上会假通过。
- **结论**：做一套不需要浏览器、用 node 直接运行的黑盒验收，对着真实 HTTP 服务器和真实 TCP 网络条件跑。
- **拍板**：用户。
- **溯源**：axp 仓库 `apps/axp/docs/reports/2026-10-27-codex-full-review.md`。

## D2 · 2026-10-27 · 服务器放在哪：改造 api-ws-demo

- **选项**：
  - A. 改造 api-ws-demo，新增 `/t/*`；
  - B. 在 codejoo 仓库里新建一个 Rust 服务。
- **结论**：A。
- **理由**：
  - 可以复用现成的 JWT、refresh、mock 能力；
  - 部署到线上后还能做冒烟测试；
  - api-ws-demo 本来就定位为"给各种客户端联调用"，dioman 等其他客户端以后可以直接接入。
- **代价**：要动另一个仓库，推送后会影响线上服务。用 `TEST_MODE` 开关加接口上限来控制风险，见 D6。
- **拍板**：用户。

## D3 · 2026-10-27 · 网络和业务分两层

- **结论**：业务场景（鉴权、失败、乱序）放在服务器的 `/t/*`；网络条件（断网、弱网、抖动）放在独立的 TCP 代理 netem。
- **理由**：
  - 断网时服务器根本收不到请求，只有代理能模拟连接被拒、黑洞、中途 reset；
  - 两层分开以后，同一个业务场景可以在每种网络预设下直接再跑一遍，不用为每种网络单独写接口。

## D4 · 2026-10-27 · netem 用 Rust 实现，作为 api-ws-demo 的独立可执行文件

- **选项**：
  - A. Node 自写的 TCP 代理，和用例在同一个进程里；
  - B. Toxiproxy（Go，外部二进制）；
  - C. Rust 实现，即 `src/bin/netem.rs`。
- **结论**：C。用户问"rust 可以做吗"之后定下来。
- **理由**：
  - 所有客户端都能复用，不绑定 Node；
  - tokio 的计时和限速比 Node 精确；
  - 服务端统一一套工具链。
- **代价**：要多走一次控制 HTTP 调用，本地大约 1ms，可以忽略。
- **约束**：Dockerfile 只编译 `--bin api-ws-demo`，netem 不进线上镜像，不影响 512MB 的内存约束。

## D5 · 2026-10-27 · 并发顺序由服务器的 barrier 决定

- **理由**：如果靠客户端各自设置 `delay_ms` 来制造先后顺序，在 weak 或 lossy 网络下会被抖动打乱，测试结果会不稳定。
  改用 barrier：服务器先把一组请求攒齐，再严格按指定顺序放行。顺序返回、乱序返回、全部成功、全部失败、部分失败都由服务器决定，
  与网络无关。

## D6 · 2026-10-27 · `/t/*` 在线上同样挂载，但设上限

- **结论**：`/t/*` 线上也挂载，以便做冒烟测试，并设置上限：barrier n≤64、hang≤60s、size≤1MB、log≤1000、delay≤10s。
  `TEST_MODE=1` 只用来关闭限流和 CPU 熔断，线上不开。
- **理由**：免费实例资源有限，不设上限的话，一个 hang 或 barrier 请求就能占住连接。
- **线上的定位**：只做冒烟，断言最终结果，不做精确计数和网络故障注入。原因是冷启动要几十秒，计数器在多个调用方之间共享，网络也注入不了。

## D7 · 2026-10-27 · 场景用与语言无关的 JSON 描述

- **结论**：场景写在 `scenarios/*.json` 里，格式由 `scenario.schema.json` 约束。每个客户端用自己的语言实现执行器。
- **理由**：
  - 用户明确要求 dioman 以后也接入这套测试；
  - 场景只写一次，多个客户端同时验收；
  - 两个库的行为不一致时，能直接暴露出来。
- **能力声明**：场景用 `requires` 声明需要的能力，缺少能力的客户端跳过该场景，不算失败。例如 axp 没有 breaker、offline、timeout。
- **预期按"正确行为"写**：预期描述的是用户真正期望的行为，不迁就任何一个库的现状。
  某个库因此失败，说明这个库有 bug。

## D8 · 2026-10-27 · 令牌失效细分为四类原因（按 D10 对齐）

- **背景**：用户要求覆盖"用无令牌、假令牌、旧令牌访问受限资源得到 401"的情况。
- **结论**：

  | code | data.reason | 含义 | 客户端行为 |
  |---|---|---|---|
  | `401` | `missing` | 无令牌 | 不刷新，判 `unauthenticated` |
  | `401` | `invalid` | 假令牌 | 最多刷新 1 次，不能循环 |
  | `401` | `expired` | 过期 | 刷新后重放 |
  | `401` | `revoked` | 旧令牌，已吊销或被顶替 | 刷新后重放 |

  另外 `code=401`、`reason=reused` 表示 refresh 被重用，服务器会吊销整个令牌族，用来暴露"并发刷新了两次"的问题。

- **配套设施**：
  - `strict_rotation` 登录选项：两种后端风格都要覆盖，即 refresh 后旧 access 立即失效，或者旧 access 在过期前仍然有效；
  - `no_refresh_` 用户：登录时只拿到 access，没有 refresh；
  - `set_token` 步骤：直接篡改客户端持有的令牌；
  - 新增错误类别 `unauthenticated`。
- **理由**：真实后端对这几种 401 的处理各不相同。客户端如果把所有 401 一律当作过期去刷新，遇到无令牌时会白白刷新一次，
  遇到假令牌时可能陷入刷新死循环。
- **拍板**：用户提出需求，Claude 定的分类。

## D9 · 2026-10-27 · 文档化要求

- **结论**：契约（CONTRACT.md，附录 A 是响应体）、测试矩阵（MATRIX.md）、设计决策（本文件）、场景索引（scenarios/README.md）
  都要落成文档，修改时同步更新。
- **拍板**：用户（"这些都要形成文档记录下来，方便溯源"）。

## D10 · 2026-10-27 · 响应风格作为测试维度；业务码对齐状态码（取代 D8 的业务码部分）

- **背景**：用户要求接口模拟两种返回：
  1. HTTP 一律 200，用 code 表达失败，例如越权调用返回 `code: 401`；
  2. 标准 HTTP 状态码，例如越权时 HTTP 401。
- **结论**：
  - 新增"响应风格"维度：`http` 和 `business`，作用于所有 `/t/*` 产生的失败。整轮用 `X-Test-Style` 头指定，单个请求用 `style=` 覆盖。
    默认每个场景两种风格都跑。
  - **code 对齐状态码**：401、403、500 等。令牌问题的细分原因放进 `data.reason`，取值为 missing、invalid、expired、revoked、reused、
    bad_credentials、forbidden、injected。**废弃 D8 原先的细分业务码**。D8 的其余内容（令牌分类、strict_rotation、
    no_refresh_、set_token、unauthenticated）继续有效。
  - 新增错误类别 `status:<n>`：与风格无关，执行器在 http 风格下按 `http:n` 匹配，在 business 风格下按 `business:n` 匹配，场景只写一份。
  - 客户端不能依赖 reason 才能工作：只看 code=401 也要能走通"刷新一次、失败判过期、不循环"。
- **选项**：
  - A. code 对齐状态码，细分原因放 reason；
  - B. 保留旧细分业务码（已否决）；
  - C. 再加一个开关，两种都支持。
- **结论**：A。
- **理由**：和用户举的例子一致，也是更常见的后端约定；细分信息不丢；不增加第三个维度，不会让场景数量翻倍。
- **拍板**：用户。

## D11 · 2026-10-27 · 补齐场景编写时暴露的契约空白

- **背景**：编写 303 个场景时，发现契约有几处没说清楚。
- **结论**：细节见 CONTRACT 附录 B，以及 §3、§4.1、§4.2 的对应条目。
  - 新增 `fail_on`：按命中序号指定哪几次失败，比只能控制前 N 次的 `fail_times` 更灵活。
  - 新增 `chunk_bytes`：`slow_body` 的分块有了确定定义。
  - `revoke` 新增 `scope`，默认 `all`，access 和 refresh 一起吊销，也就是真正的踢下线，客户端应判 `expired`；`access` 只吊销 access，刷新后可以恢复。
  - 新增 `/t/mock/*`，给 mock 类插件当 mock 服务用。
  - status 与 code 同时给出时的分类规则。
  - login 等准备步骤走旁路，因此带登录的场景也能跑断网档。
  - `previous` 的精确定义。
  - 新增 `style` 步骤，可以在运行中切换响应风格。
- **理由**：这些地方不明确，同一个场景在不同客户端的执行器里会有不同理解，跨语言验收就失去了意义。
- **拍板**：Claude，根据场景作者反馈的缺口整理。

## D12 · 2026-10-27 · 网络模拟与观测的实现边界

- **背景**：在不新增 crate、沿用免费实例资源限制的前提下，实现真实连接故障和取消观测。
- **结论**：
  - 响应生命周期守卫覆盖 handler future 与响应体流；客户端提前断开时由 Drop 写入 aborted，服务端主动注入 close/half_body 单独标记。
  - 探针和统计管理不写业务计数；barrier 等待前一响应体结束或取消，再按 gap 放行下一请求。
  - netem 使用 4096B 缓冲和按连接、按方向的令牌桶；kbps 为十进制千比特每秒；reset 使用 Tokio 的 SO_LINGER=0。
  - refuse 实际关闭监听，因此用户态无法统计每次被内核拒绝的尝试，refused 记录进入拒绝监听状态的次数。
  - Windows 连接重试可能跨越 flap 的恢复时刻；时间表测试在离线窗口内做短时探测，持续 refuse 测试单独断言 ConnectionRefused。
- **验证边界**：本地 Cargo 离线缓存缺少原有 governor 等锁定依赖，完整测试和构建需要补齐缓存；独立缓存校验只作为核心逻辑的补充证据。
- **来源**：本轮用户要求与实现过程中真实 TCP 测试结果；具体澄清已标注在 CONTRACT 对应章节。

## D13 · 2026-10-27 · 完整接入 D10 与 D11

- **背景**：用户明确要求以当前 CONTRACT 为准，清理旧码和旧响应风格参数。
- **结论**：统一所有 `/t/*` 失败的风格、code、reason 和消息；严格轮换按会话族吊销旧 access，revoke 按用户和 scope 吊销。
  吊销数量只统计本次新失效且未过期、未使用的令牌；日志增加 style 和 reason。
- **理由**：跨语言场景需要一致的响应和可核对的服务端记录。
- **拍板**：用户。


## D14 · 2026-10-27 · dioman Dart io 执行器接入与公开 API 探针

- **背景**：用户要求 dioman 读取中立场景，复用已启动的真实服务与 netem，公开选项/方法逐项覆盖并验证插件任意组合。
- **结论**：执行器放在 dioman/accept，17 种能力、16 个 step 均有映射。未知字段显式报错；缺能力按 missing 跳过。上游使用 127.0.0.1:28080，netem 控制地址 127.0.0.1:9090，每轮唯一 X-Test-Run。
- **职责边界**：连接/收发超时由 DiomanTimeout 设置；逻辑总预算由执行器 Timer 管理。离线探测是宿主输入，DiomanOffline 负责队列及拒绝。业务信封在转换器中提前转成原始响应错误，使 auth/retry/cache 能按同一种逻辑失败组合。
- **场景修正**：expire_local 在执行器中只更换 access；previous 场景先建立真正存在的旧值，撤销 access 后保留当前的被撤销令牌；201 成功显式给 code=0；429 重试显式给状态白名单；慢刷新 cancel_all 补装 cancel；离线恢复场景要求恢复后的请求成功。以上均依据现有契约，不迁就客户端失败。
- **扩展**：新增 combo.all-seventeen-plugins；dioman 专用真实幂集探针覆盖全部 131071 个非空子集，逐项检查 payload、服务端 hit 与资源释放。API AST 清单、逐字段读取与源码 lcov 分开报告，未运行不得宣称通过。
- **当前核实状态**：normal×http 基线 300 通过、0 失败；新增 5 个库级回归通过。全矩阵运行中 28080 服务停止监听，9090 仍可达且 PID 已改变；已停止本次执行器请求，未启动、重启或关闭这两个服务。后续完整矩阵与 API 100% 以实际补跑报告为准。
- **拍板**：用户授权接入、修库与扩展场景；具体映射和证据边界由本次执行实现。


## D15 · 2026-10-27 · dioman 本轮证据边界与中断结果

- **已核实**：normal×http 历史基线 300 通过、0 失败；最终全量库级测试 290 通过、4 跳过，耗时 8 分 42 秒。dart analyze 无问题；328 个中立场景通过 validate --check-index（退出码 0）。
- **覆盖指标**：最终单测 lcov 为 1683/1793 行，即 93.8650%；这是库级行覆盖，不代表真实服务公开 API 覆盖。AST 清单 691 项；专用真实 API 探针未完成，不能声称 API 达到 100%。17 插件真实幂集也未运行。
- **矩阵中断**：现有场景计划 1059 项；本轮全矩阵执行 358 项，其中 210 通过、148 失败。服务停止监听后剩余 701 项标为跳过/未执行，不计为能力缺失，也不把全部失败归咎于 dioman。原始记录和审计见 dioman/accept/results/。
- **库修复**：并发过期去重及稳定过期错误、重放仍 401 的会话结束、retry 遇 401 交回 auth（保留原始裸令牌）、每次重试计入 breaker、恢复响应写回 cache、share 首发取消交接给存活跟随者、notify 置于 normalize 前且同一逻辑请求仅通知一次。新增 7 个独立回归均通过。
- **资源边界**：未操作两个服务进程。强制中断历史执行器时最后若干代理没有完整 ID 日志，清理状态不能全部核实；未盲删未知代理。后续版本已加入创建/删除日志和基础设施不可达时中断逻辑。
- **未完成**：服务恢复后的最终修订正常网络复验、全网络/风格矩阵、逐项 API 真实证据及 131071 个真实插件子集。以实际后续补跑报告为准。


## D16 · 2026-10-27 · dioman 恢复验收的最终证据（补充 D14、D15）

- **背景与授权**：用户要求保留已有改动、自行执行、不转派、不 commit/stash，复用已启动服务。仅修改 dioman 与本 testkit。
- **端口**：本轮实际使用 127.0.0.1:47080 / 47090。执行器支持 TESTKIT_SERVER / TESTKIT_NETEM 完整 HTTP 地址覆盖，上游由服务器地址派生；未启动、重启或停止服务。
- **矩阵口径与取舍**：328 场景 × 7 网络 × 2 风格 × io 共 4592 个结论：1045 通过、0 失败、3547 跳过。其中 3533 个组合未被场景声明，另 14 个声明组合主动注入 429/503，与本轮遇到即停约束冲突，明确跳过。未将这些项计为通过。
- **失败复查**：上轮 148 项失败本轮 144 通过、4 约束跳过、0 失败。本轮首次完整执行 1027 通过、18 失败、3547 跳过；定位并修正场景/执行器后，第二次完整矩阵 4592 项已执行到最终结果。原始记录保留在 all-resume.json，逐项分层与最终 run id 见 failure-triage.json。
- **公开 API**：691/691，100% 的构造、读取、调用登记覆盖；真实服务轮次 dioman-api-1793099269471162，附 47080 服务端日志。metadata/兼容 API 在同一真实请求上下文中验证，不宣称它们自身发送 HTTP，也不代表所有参数值/分支均覆盖。
- **组合策略**：17 个单开 + 136 个精确双开 + 全部同开 = 154 个真实组合，154 通过、0 失败。每子集独立 X-Test-Run，经过真实代理，校验 payload、hit=1、inflight=0 和 loading 释放。可复现命令见 dioman/accept/README.md；其余 130917 子集未执行，不宣称全幂集完成。
- **场景修正依据**：Windows 连接拒绝与取消存在竞态；无重试 flap 对照组需短连接预算，避免系统 TCP 尝试跨过恢复窗口；offline/DNS 与 flap 不固定 loading 次数；blackhole 已接受 TCP 连接，应验证接收超时。未修改服务端契约或实现，只使用已有 schema 能力。
- **执行器修正**：flap 宿主探测使用 500ms 预算；逐项结果增量追加 NDJSON、结束后汇总 JSON，避免同步反复编码完整历史阻塞事件循环，导致短 TTL 或在途窗口断言被执行器污染；修正 API 探针 filter 删除谓词方向。
- **库状态**：本轮未新增修改 lib/；上轮七项库修复的独立回归全部复验通过。dart test 最终 290 通过、4 跳过、0 失败；dart analyze 无问题；node tools/validate.mjs --check-index 退出码 0，328 场景有效。上轮 lcov 数值仍为历史指标，未宣称本轮重采集。
- **资源**：本轮代理日志中 1984 个创建 ID 均有删除成功记录。早期未记录 ID 的历史代理未核实归属、未盲删。
- **最终产物**：dioman/accept/results/audit.json、api-coverage.json、api-evidence.json、powerset-summary.json、failure-triage.json、verification.json、proxy-cleanup.json、changed-files-resume.json。
- **拍板**：用户授权的续跑、场景扩展与修复；执行策略与证据边界由本次执行记录。


## D17 · 2026-10-27 · dioman 五方法复核及共享异常结算修复（部分完成）

- **背景与授权**：用户要求自行执行、不转派、不commit/stash，保留原改动，复用47080/47090；仅修改dioman和本testkit。未操作服务进程，未增加依赖。
- **跳过复核**：历史3547格 = 3477纯缺声明 + 56缺声明且主动注入429/503 + 14已声明注入429/503。5个注入场景对全部7网络写expect.skip，现70格有明确约束理由；其中56格补齐原缺声明。其余3477格未补齐预期并真实执行，不能宣称合规不适用或已验收。拆出安全状态码子矩阵和HTTP/业务码优先级子场景，8格真实通过，不代替429/503专门证据。
- **方法矩阵**：新增79场景，单体34、全量6、部分边界56、配对64，共160格最终真实通过，0失败、0跳过；normal/weak/flap全开业务流覆盖两种风格。规则随实际请求记录，完整安装图逐边逆序拒绝、重复名拒绝及extra唯一/归属/前缀检查均执行。规则不重复计HTTP请求。
- **配对选择**：搜索现有依赖后未新增第三方库；采用32×17二元正交数组最小实现，2个生成器单测独立枚举136×4状态。旧136双开集合数学上可覆盖四种开关状态，但旧证据仅http；本轮32行各跑http/business。
- **新缺陷及修复**：share内部重试的重放准备（如自定义FormData.clone抛FormatException）进入异常兜底时只结束首发，未结算共享entry，使跟随者悬挂。修复前回归失败；补_finishOrDefer后单测和真实http/business各1格通过，服务端各命中1次、inflight=0、loading成对。另修正flap中offline早于notify拒绝导致的通知次数预期，两次首次失败已复验通过。
- **库自审与覆盖**：上轮七文件diff逐项关联回归、顺序及extra注册；删除key私有_encode仅Map调用条件下的不可达非Map分支；修正README/Breaker类注释与逐尝试计数源码的冲突。原110行逐项处置：103行新命中、1行不可达删除、1行由缺陷修复替换且新出口覆盖、5行保留兜底未命中。最终lcov 1788/1793 = 99.7211%，5行逐条给出标准Dio契约下的不可达理由并保留依赖源码摘录，不计为覆盖。
- **最终检查**：dart analyze无问题；dart test 330通过/4跳过/0失败，耗时08:25，已等到进程退出码0；新增缺口单测38个、pairwise生成器单测2个。node tools/validate.mjs --check-index退出码0，18文件409场景。公开API本轮重新执行691/691，run id为dioman-api-1793104721774626，不代表参数取值/触发路径穷举。
- **资源**：本轮结果涉及的494个自建代理全部有删除成功记录；未盲删未知历史代理。每轮唯一X-Test-Run，无已观察到的429/503或服务不可达。
- **尚未完成**：3477缺声明组合真实补跑；单体全部公开选项和触发路径；全部数值/时间/集合边界；全部源码不变量规则化。方法矩阵、覆盖和API指标分别报告，不将部分已实现矩阵通过称为整项完成。
- **产物**：dioman/accept/MATRIX-plugins.md、COVERAGE-review.md、REVIEW-library.md、results/review-summary.json、verification-review.json、coverage-baseline-resolution.json、coverage-review.json、skip-review.json、plugin-matrix-final.json、share-prepare-real.json、proxy-cleanup-review.json。
- **拍板**：用户授权修库与扩展测试；本轮实际完成范围与未完成事项由证据限定。


## D18 · 2026-10-27 · 测试整理

- **背景与授权**：用户要求自行整理 dioman 测试与本 testkit 场景，仅分类、归纳、去重，不新增测试能力、不改 lib/ 行为；不转派、不 commit/stash/reset/checkout，复用 47080/47090。
- **选择与结论**：dioman 按 unit/rules/boundary/combo/integration/support 归类；accept 分 bin/lib/docs/results。单体和边界归入对应领域，plugins.json 保留配对/全量。严格比较网络、风格、步骤、断言和执行器缺省值后，合并 retry 两个零值场景及 loading 两个零值场景，保留 id 显式传两个选项；旧 id 映射在 dioman/test/README.md，生成器与方法清单同步。其余不同插件集合、边界输入和仲裁路径保留。
- **结果**：409→407 场景，18 文件；gen-index 与 validate --check-index 退出 0。normal×http 376通过/33跳过→374通过/33跳过，均0失败；五方法160→156格通过、0失败。通过数减少完全对应两个去重场景及双风格格子，按旧→新 id 比对无缺失、无状态变化，未把原始格数减少隐瞒为不变。
- **库级检查**：旧334运行时用例（330通过、4跳过）→328（本地325通过、real3通过）；5条重复合并、1条空跳过占位删除，映射无意外丢失/新增。行覆盖1788/1793保持不变，dart analyze无问题。默认排除real；因 test 排除优先于包含，真实命令为 dart test -t real --configuration=dart_test.real.yaml --fail-fast，单独 -t real 已验证退出79，不宣称符合原命令字面要求。
- **证据与边界**：lib/ 与原 accept/results/ 哈希不变；新证据在 dioman/coverage/accept/，整理前后对比在 coverage/reorganization/，分类/全映射/文件清单在 test/README.md。未操作服务进程，未观察429/503或不可达。整理后532个自建代理中530个有删除日志，另2个逐 id GET 返回404确认不存在；日志缺项原因未定位，未盲删未知代理。选项/非法值/不变量的穷尽覆盖与历史未声明组合仍未补齐。
- **拍板**：用户确定目录和去重规则；具体迁移与原生 test 配置差异按本条及映射表如实记录。


## D19 · 2026-10-28 · filter 请求级开关与"假通过"修复

- **背景**：场景 schema 的 `request.options` 没有 `filter` 字段，执行器无从映射，`client.plugins.filter` 装配的插件靠请求级 `config.filter` 触发，所以验收里 filter 从未真正生效。`filter.keeps-zero-false`、`key.filter-before-key` 等场景的"通过"是假通过（前者的子集匹配也看不出空值有没有被剔除）。
- **选项**：A. schema 加 `options.filter: boolean`，场景显式开启，并按服务端实收内容断言；B. 只给相关场景标 requires、实际仍不生效，靠跳过规避。
- **结论**：选 A。CONTRACT §7 定义 `filter` 的中立语义（请求级开启，剔除 query/JSON 对象 body 顶层的 null/undefined/NaN/空白字符串，保留 0/false，不递归）；`ignoreKeys`/`ignoreValues`/自定义 predicate 不在中立表达范围。新增 `scenarios/filter.json`，旧的两个 filter 场景补 `options.filter: true`。
- **理由**：B 不解决问题，只是把假通过换成不运行；A 让能力和断言对得上，不认识该选项的执行器仍可按"缺能力/映射不了"处理。断言落在 echo 的 `body.raw`（精确串，可证明缺席）上；query 因 echo 的 query 是对象子集匹配，缺席只能靠 `key.filter-before-key` 的共享计数佐证。plugins.json、combos.json 里仅装配 filter 的场景保持不变，它们验证"装配不破坏其它能力"。
- **拍板**：用户（jelon），2026-10-28。


## D20 · 2026-10-28 · 撤销"遇 429/503 即停"的临时 skip，按真实预期执行

- **背景**：D17 那一轮因"遇 429/503 立即停止"的临时约束，把 `retry.429-retried`、`retry.503-retry-after`、`retry.respects-retry-after-over-delay`、`basic.status-matrix`、`faults.status-with-business` 在全部 7 个网络上都写成 skip。约束只属于那一轮，却留在了共享场景里，所有执行器都被迫跳过这 5 个场景。
- **选项**：A. 保留 skip；B. 按正确行为写预期，确实无法确定性断言的网络单独写原因；C. 缩小 `networks` 去掉那些网络。
- **结论**：选 B。normal 恢复原有的精确预期；weak/lossy 只断言最终结果与资源释放；offline/dns 断言 `network` 且服务端 total=0，blackhole 断言 `timeout`（场景补 `base_timeout_ms=4000`、`timeout_ms=40000`）。仍写 skip 的只有：lossy（15% 随机 RST 与被测行为无关，`status-matrix`、`status-with-business`、`respects-retry-after-over-delay`），flap（离线窗口 1500ms 长于重试预算，或单发请求只会得到 network）。skip 原因都写在场景里，不再用"本轮用户要求"。`status-with-business` 的 `normal`/`weak` 现在断言 http 风格 `status:503`，`@business` 断言 `business:9`（附录 B.1）。
- **Retry-After 格式**：读服务端实现（`src/testkit/faults.rs`、`mod.rs`）确认，`retry_after` 参数按 u64 解析并原样写入头，只能产生整数秒（delta-seconds），**无法产生 HTTP 日期形式**；CONTRACT §3 已写明。日期形式的验收只能靠客户端单测，要在黑盒验证需要服务端新增能力（本次不改 Rust，作为缺口汇报）。
- **理由**：D17 自己写明这 5 个场景是临时约束的产物，70 格 skip 不能算验收。"遇 429/503 即停"的目的是防真实线上限流，而这几个场景的 429/503 是服务端注入的（`data.reason == "injected"`，附录 B.6 记录了这条区分方法），执行器可据此放行，不必整体跳过。
- **拍板**：用户（jelon）授权，主会话代拍，2026-10-28。


## D21 · 2026-10-28 · plugins.single-auth 在 business 风格下的预期：ok:true

- **背景**：`plugins.single-auth` 只装 auth、不装 normalize，发一个 `status=500&code=500` 的请求。D18 把单体场景并入领域文件时它进了 `auth.json`，`normal@business` 的预期是 `ok: true`（git 里最早出现在 9820bdb，此前没有）。但 dioman 的 `test/accept/bin/build_plugin_matrix.dart` 对 `normalize` 和 `auth` 都生成"business 下 500 识别为失败"（`ok:false, status:500`），与已提交的场景不一致；dioman 的 auth 在装配时还会启用信封转换器，所以 business 下 500 被识别为 `DiomanException(code: 500)`，这一格对 dioman 失败。
- **选项**：A. 保持 `ok: true`（只装 auth 时不识别一般业务失败）；B. 改成 `ok:false, status:500`（auth 也负责识别所有业务失败）。
- **结论**：选 A，场景预期不变，只在 `why` 里写清楚。
- **理由**：AGENTS 的约定是"business 风格下判定失败依赖 normalize 类插件"；CONTRACT §4.2 与 schema 的 `pluginOptions.auth` 只要求 auth 在两种风格下识别 **401/403**（刷新、越权），没有要求识别 500。一个 500 信封是 HTTP 200 + code 500，对不装 normalize 的客户端就是"成功的 HTTP 响应"，其余单体场景（cache、retry、share 等）在 `normal@business` 也都是 `ok: true`，只有 `plugins.single-normalize` 是 `ok:false`。axp 执行器同样如此（未装 normalize 时 auth 只在过期/越权后把失败响应原样 resolve）。
- **需要 dioman 侧跟进（不在 testkit 改动范围）**：把 `build_plugin_matrix.dart` 里 `name == tkNormalize || name == tkAuth` 改为只对 normalize 生成 `status:500`，否则重新生成会回退预期；dioman 执行器在只装 auth 时不应启用对 500 信封的失败识别（auth 仍要识别 401/403）。
- **拍板**：用户（jelon）授权，主会话代拍，2026-10-28。


## D22 · 2026-10-28 · 发送超时落地，连接超时暂不实现

- **背景**：dioman 的 H5 假设（插件改写的超时配置真的在对应阶段生效）需要真实核对。现有 `timeout.connect-blackhole` 测的其实是接收超时：netem 的 blackhole 在用户态完成 TCP 握手后才不转发，连接已建立，`connect_ms` 没有机会触发。
- **发送超时（落地）**：schema 的 `client.plugins.timeout` 增加 `send_ms`，新增能力 `timeout-send`（CONTRACT §7），场景 `timeout.send-slow-reader`：POST 一个由 seed 生成的 multipart 大文件，`read_delay_ms=5000`、`send_ms=500`，断言 `timeout` 且 `duration_ms` ≤ 2000。大请求体复用 multipart 的 seed 机制，不另开新的 body 类型，所以同时需要 `multipart` 能力。**请求体取 16MiB 而不是提议里的约 8MB**：在本机（Windows 回环 + netem 一跳）实测，Node 流式写入约 5.6MB 后才阻塞（直连约 2.8MB），8MB 只比阈值多 40%，换个平台或缓冲调优就可能被吞掉；16MiB 约 3 倍裕量，生成耗时约 0.2s。另外，一次性把整块请求体交给内核的 HTTP 栈（Node 对 256MB 的 write 也在 100ms 内返回）发送阶段永远不阻塞，没有可观察的发送超时，不具备 `timeout-send`。
- **连接超时（暂不实现）**：服务端上一步汇报：netem 丢 SYN 不可行。代理只能 accept 内核已完成握手的连接，停止 accept 也挡不住握手；跨平台无法可靠控制；新增测试证明 blackhole 是握手完成后才读超时。选项：A. netem 用防火墙或原始 socket 丢 SYN（需要管理员权限和平台专用依赖）；B. 单独开不响应的端口（不 listen 会得到 RST，listen 不 accept 内核仍会完成握手，只有 backlog 满了才丢 SYN，依赖平台）；C. 客户端单测用 pending 连接器覆盖。结论：黑盒不测 connect 超时，由 C 覆盖；不加 `timeout.connect-nosyn`。`timeout.connect-blackhole` 保留 id，改标题和 why，写明它测的是已连接后的接收超时，`connect_ms=200` 小于 `receive_ms` 是为了证明连接预算不会误伤已建立的连接。重新评估的条件：能接受需要管理员权限的包过滤，或有稳定的不可路由地址。
- **拍板**：用户（jelon）授权，主会话代拍，2026-10-28。


## D23 · 2026-10-28 · 落地 dioman 缺口提议：新场景与细分能力

- **背景**：dioman 的"② 单测 + 真实核对"找不到 testkit 场景核对的能力，提议见 `proposals/2026-10-28-dioman-gaps.md`（已按本条落地，文件改为"已落地"状态）。这些能力对所有执行器都有意义，不是 dioman 特有。
- **结论与各节去向**：
  - **熔断半开探测被取消**：只新增场景 `breaker.half-open-probe-cancel`，无契约改动；补了 f1–f3 的失败预期，服务端 6 次（3 次失败 + 被取消的探测 1 次 + 探测 2 + after）、`aborted ≥ 1`。
  - **multipart**：`request.body` 支持 `{"multipart": {fields, files[{name, filename, size, seed}]}}`，新能力 `multipart`，场景 `auth.expired-multipart-replay`（用 `expire_local` 触发 401，重放后断言 echo 回报的每个文件 size 与 sha256，服务端 `/t/private/upload` 恰好 2 次、刷新 1 次）。**seed 生成算法选 32 位线性同余**（`state = state*1664525 + 1013904223 mod 2^32`，取高 8 位），CONTRACT §7 给出自检向量。选项对比：sha256(seed‖i) 拼接要求每个执行器引入哈希库（dio 默认没有）且更慢；LCG 各语言 5 行、无依赖、16MiB 约 0.2s。期望的 sha256 在场景里写死，执行器无需再算一遍对比。
  - **连接复用**：服务端已给 `meta.conn`。场景 `basic.keep-alive-recovery-isolated`，新能力 `keep-alive`。失败响应在执行器里拿不到 `data`，不能直接比较失败请求的连接，所以改成"成功 warm → 401 恢复失败 → 500 重试耗尽 → 成功 final"，要求 warm 与 final 的 `meta.conn` 相同。schema 最小扩展：结果项 `same_as: {id, field}`（比较两个成功请求 data 的同一字段）；不选"服务端 `runs` 日志带 conn"，因为要改 Rust，可作为以后的增强。`tools/validate.mjs` 校验 `same_as` 引用的 id 存在、不自引用、只用于 ok:true。
  - **share retry 策略**：CONTRACT §7 增加 `share.policy: retry`（共享同一结果、失败时内部重试、调用方只见最终一次；取消不重试；次数和间隔由执行器映射到客户端默认，契约只要求默认至少重试 1 次；请求体必须可重放，无法重放就不重试、不得发空体）；schema 的 policy 与 `options.share` 加 `retry`；新能力 `share-retry`；场景 `share.retry-recovers-single-result`（3 并发、`fail_on=1`，都成功、服务端恰好 2 次）。
  - **mock 组合语义**：CONTRACT §7 写明硬要求（mock 响应不写缓存、不污染同 key 的真实请求，`combo.cache-mock-no-key-collision` 所有 mock 执行器都跑）与可选语义（新能力 `mock-in-chain`：mock 探测在插件链内、不走 cache/share，只有回落后的真实请求才走）。`combo.cache-mock-fallback-cached-once`、`combo.mock-share-fallback-single-flight` 的 requires 加 `mock-in-chain`，三个场景的 why 改成中立表述，去掉对 axp 源码文件的引用。mock 在传输层实现的执行器（如 dioman 的 DiomanMockAdapter）按缺能力统一跳过，不用各自特判。
  - **发送/连接超时**：见 D22。**429/503 skip 与 single-auth**：见 D20、D21。
- **新增能力名**：`multipart`、`keep-alive`、`share-retry`、`timeout-send`、`mock-in-chain`（schema `$defs/capability` 与 CONTRACT §7 同步）。`tools/validate.mjs` 增加一致性检查：场景用了 multipart 请求体、`share.policy: retry`（含请求级 `share: "retry"`）或 `timeout.send_ms` 却没在 `requires` 声明对应能力时报错，避免缺能力的执行器漏跳过。
- **未落地**：netem 丢 SYN 的 connect 超时（D22）；HTTP 日期形式的 Retry-After（D20，服务端无此能力）。
- **拍板**：用户（jelon）授权，主会话代拍，2026-10-28。
