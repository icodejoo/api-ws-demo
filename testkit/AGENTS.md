# testkit 手册（写给 AI）

testkit 是跨语言的 HTTP 客户端黑盒验收工具，分三层：**场景**（`scenarios/*.json`，与语言无关）、**netem**（网络条件代理）、**`/t/*`**（业务场景接口）。
所有改动都遵循**契约先行**：先改契约文档，再改实现和场景，最后记录决策。

先判断你属于哪个分支，只读那一节。

---

## 资源地图

| 要找什么 | 在哪 |
|---|---|
| 接口行为、控制参数、业务码、响应体 | `CONTRACT.md`（§2.1 响应风格，§4.2 鉴权，附录 A 响应体，附录 B 语义澄清） |
| 覆盖哪些维度、每种网络下断言什么 | `MATRIX.md` |
| 为什么这样设计、谁拍的板 | `DECISIONS.md`（D 编号，只追加不修改） |
| 场景格式 | `scenario.schema.json` |
| dioman 测试分类与迁移映射 | `E:\workspaces\dart-labs\dioman\test\README.md`；本地 `dart test`，真实服务 `dart test -t real --configuration=dart_test.real.yaml --fail-fast` |
| 场景本身 | `scenarios/<领域>.json`；按 id 查找：`grep -rn '"id": "<id>"' scenarios/` |
| 场景索引与覆盖统计 | `scenarios/README.md`，由 `tools/gen-index.mjs` 生成 |
| axp API 反查与组合缺口 | `COVERAGE.md`；清单 `coverage/axp-api.json`，生成/检查：`node tools/gen-coverage.mjs [--check]` |
| 校验 | `node tools/validate.mjs --check-index` |
| `/t/*` 的服务端实现 | `../src/testkit/`，路由在 `../src/routes.rs` |
| netem 的实现 | `../src/netem/`，入口在 `../src/bin/netem.rs` |
| 服务端集成测试 | `../tests/testkit.rs`、`../tests/netem.rs` |
| 已接入的客户端执行器 | axp：`E:\workspaces\codejoo\apps\axp\accept\`（执行 `npm run accept`） |
| dioman Dart io 执行器 | `E:\workspaces\dart-labs\dioman\accept\`（`dart run accept/bin/cli.dart`；映射见 `accept/docs/mapping.md`，新证据见 `coverage/accept/`，历史结果保留在 `accept/results/`） |
| dioman 最终验收证据 | `dioman/accept/results/audit.json`、`api-coverage.json`、`api-evidence.json`、`powerset-summary.json`；代理归属见 `proxies.ndjson`，逐项失败原始记录见 `failure-review-initial.json` |

---

## 分支一：接入新客户端

1. 读 `README.md` 里的"接入新的客户端"一节、`CONTRACT.md` §7 和附录 B，再读 `scenario.schema.json`。
   **完成标准**：能说清楚执行器要实现的全部 step 类型、全部错误类别和全部客户端事件，每一个都有对应项。
2. 列出该客户端具备的能力（CONTRACT §7 的能力名）。缺失的能力不用实现，执行器输出 `skipped: missing <cap>` 即可。
3. 写**能力映射表**，放在该客户端的执行器目录里，有三张：
   - 中立插件选项 → 本库参数；
   - 本库错误 → 错误类别；
   - 本库回调 → 客户端事件。
   映射不了的选项要显式报错。
   **完成标准**：schema 中 `pluginOptions` 的每个字段都标明了"已映射"或"显式报错"。
4. 实现执行器。参考 axp 的执行器，它是第一个落地的实现。
5. 先跑 `normal × http`，再逐步放开全部网络和风格。
   **完成标准**：每个场景都有明确结论，只能是通过、失败、跳过三种之一，失败的附上原因。
6. 在本文件的资源地图里加一行该客户端执行器的位置，在 `DECISIONS.md` 追加一条接入记录。

## 分支二：扩展测试范围

1. 判断这次变化落在哪一层：
   - **新的服务端行为**（新接口、新控制参数、新故障）：改 `CONTRACT.md`，同时补附录 A 的响应体。
   - **新的场景表达能力**（新 step、新断言字段、新错误类别）：改 `scenario.schema.json` 和 CONTRACT §7。
   - **只是新增场景**：直接进入第 3 步。
2. 契约改动要同步实现：服务端在 `../src/testkit/` 或 netem，并补上 `../tests/` 下的测试；所有执行器都支持新的 step 或字段。
   **完成标准**：`cargo test` 通过；契约里每一条新增内容都能在实现中找到对应代码。
3. 写场景时，预期按**正确行为**写，不要迁就某个客户端的现状：
   - 与风格无关的错误用 `status:<n>`；
   - 不适用的网络写 `skip` 并说明原因；
   - 每个场景都要写 `why`。
4. 执行 `node tools/gen-index.mjs`，再执行 `node tools/validate.mjs --check-index`。
   **完成标准**：两条命令的退出码都是 0。
5. 改了覆盖维度就更新 `MATRIX.md`；做了取舍就在 `DECISIONS.md` 追加一条记录，写明背景、选项、结论、理由和拍板人。

## 分支三：排查失败

1. 先定位失败落在哪一层：

   | 现象 | 先看哪里 |
   |---|---|
   | 场景校验不过、索引过期 | `tools/validate.mjs` 的报错信息，以及 schema |
   | 服务端行为和契约不符，例如 code 错了、风格没生效、barrier 放行顺序不对 | `../src/testkit/` 对照 CONTRACT 相应章节；用 curl 带 `X-Test-Run` 请求，再查 `GET /t/runs/{run}` 的 log |
   | 网络条件没生效 | netem 的 `GET /proxies/{id}/stats`（refused、resets、bytes 计数）对照 CONTRACT §6 |
   | 客户端结果和预期不符 | 先确认场景预期是不是按正确行为写的；再看执行器的能力映射表；最后才看客户端库本身 |
   | 只在 weak、lossy、flap 网络下偶发失败 | 检查场景有没有违反 MATRIX §2，即在这些网络下断言了耗时、计数或顺序；或者 seed 没有固定 |
   | 只在 business 风格下失败 | 客户端是否只看 HTTP 状态码。business 风格下失败响应是 HTTP 200 加 code，必须在响应体层识别 |

2. 用服务端 log 复现。每次运行都有唯一的 `X-Test-Run`，`GET /t/runs/{run}` 的 log 能看到每个请求的 style、status、code、reason，以及请求是否被中止（aborted）。
   **完成标准**：能用一句话说清楚问题出在哪一层、具体是哪个文件。
3. 根因在客户端库时，到对应仓库修复并补单测；根因在契约或场景时，按分支二的流程修改。

---

## 本地运行与端口

默认端口特意选了偏门的，避开 8080/9090/3000/5173 这类常见开发端口：

| 服务 | 默认地址 | 改端口的方式 |
|---|---|---|
| 测试服务器 `api-ws-demo` | `127.0.0.1:47080` | 环境变量 `PORT` |
| netem 控制面 | `127.0.0.1:47090` | 参数 `--control <addr>` |

启动（先服务器、后 netem，各开一个终端）：

```bash
TEST_MODE=1 cargo run --release
cargo run --release --bin netem
```

执行器（以 axp 为例）默认连上面两个地址；改了端口就用环境变量对齐：`TESTKIT_SERVER`、`TESTKIT_NETEM`。
创建 netem 代理时 `upstream` 要写成测试服务器的实际地址。

**端口被占时**：启动会报 `AddrInUse`，换个端口即可，不要去杀占用进程，那多半是别的项目的开发服务。
注意 `0.0.0.0:<端口>` 和 `127.0.0.1:<端口>` 可以各被一个进程占住，后者优先，客户端连到的可能是别人的服务。
所以启动后先 `curl http://127.0.0.1:<端口>/t/ping`，确认返回 `{"code":0}` 再跑验收。

## 踩过的坑

- **环境变量 `CARGO_TARGET_DIR` 会让产物不落在仓库的 `target/`**。改完代码后 `target/release/*.exe` 可能是旧的，启动后端口还是老的。编译用 `cargo build --release --target-dir target`，并看启动日志里打印的端口确认。

- **含 login 的步骤走旁路**，不经过 netem，也不经过客户端插件链（附录 B.2）。所以断网档下登录照样能成功，失败的只有被测请求。
- **business 风格下判定失败依赖 normalize 类插件**。场景作者已经给相关场景自动加上了 normalize，客户端缺少这个能力时应该跳过，不能算失败。
- **`scenarios/README.md` 不要手改**，要改就改 `tools/gen-index.mjs`。
- **`/t/*` 在线上也挂载着**，但线上只适合做冒烟测试。冷启动、限流、共享计数器都会让精确断言失效（D6）。本地运行时要加 `TEST_MODE=1`。
- **netem 不进 Docker 镜像**。Dockerfile 只构建 `--bin api-ws-demo`，新增可执行文件时要保持这一点（D4）。


## 2026-10-27 · dioman 五方法复核（部分完成）

本轮入口：dioman/accept/bin/build_plugin_matrix.dart 生成 scenarios/plugins.json；
dioman/accept/bin/run_plugin_matrix.dart 执行方法清单；中文逐格矩阵在 dioman/accept/docs/MATRIX-plugins.md。
完整汇总为 dioman/accept/results/review-summary.json，收尾检查为 verification-review.json；
D16 的 audit.json 保留为历史证据，不代表本轮补齐全部缺声明组合。
新增79个方法场景和2个安全状态子场景后共409场景；校验命令保持 gen-index / validate --check-index。
历史跳过3547格复核后，70格在全部网络显式说明禁止主动注入429/503，其中56格原来缺声明；
其余3477格仍未补齐预期和真实执行。单体全部选项、全部原生边界及全部实现不变量尚未穷尽。
share重放准备异常导致跟随者悬挂的修复，真实双风格证据在 share-prepare-real.json。
不得仅因方法清单160格通过，就将整项复核报告为完成。

## 2026-10-27 · 测试整理资源定位

单体与边界场景已归入对应领域文件，plugins.json 保留配对和全量场景；五方法清单在 dioman/accept/lib/plugin_matrix_plan.json。两组等价零值边界已合并，旧 id 映射见 dioman/test/README.md。现有 18 个文件共 407 场景。原 results 证据不重写，新执行证据在 dioman/coverage/accept/。
