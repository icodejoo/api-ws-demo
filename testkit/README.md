# testkit：HTTP 客户端黑盒验收

这是给 HTTP 客户端插件库用的跨语言验收设施，不需要浏览器，在 node、dart 等环境里直接运行。
它对着真实的 api-ws-demo 服务器和 netem 网络代理跑测试。

- 已接入：axp（`codejoo/apps/axp`）。
- 计划接入：dioman（`dart-labs/dioman`）。

## 文档索引

| 文档 | 内容 | 什么时候改 |
|---|---|---|
| [AGENTS.md](./AGENTS.md) | **写给 AI 的手册**：资源地图，以及接入新客户端、扩展测试范围、排查失败三个分支的操作步骤 | 资源位置或流程有变化时 |
| [CONTRACT.md](./CONTRACT.md) | 接口契约：`/t/*` 接口、控制参数、坏响应注入、鉴权业务码、barrier、观测、netem 控制 API 与网络预设；附录 A 是各接口的响应体 | 接口行为变了，先改这里，再改实现 |
| [MATRIX.md](./MATRIX.md) | 测试组合矩阵：覆盖哪些维度，以及不同网络下怎么断言 | 增删覆盖范围时 |
| [DECISIONS.md](./DECISIONS.md) | 设计决策记录：背景、选项、结论、理由、谁拍的板 | 有新决策，或推翻旧决策时，只能追加 |
| [scenario.schema.json](./scenario.schema.json) | 场景 JSON 的格式 | 场景表达能力不够时 |
| [scenarios/README.md](./scenarios/README.md) | 场景目录说明、覆盖统计、逐条索引 | 增删场景时 |
| `tools/validate.mjs` | 场景校验脚本 | – |

## 本地运行

```bash
# 1. 测试服务器，TEST_MODE 会关闭限流和 CPU 熔断
TEST_MODE=1 cargo run --release

# 2. 网络代理
cargo run --release --bin netem -- --control 127.0.0.1:47090

# 3. 客户端执行器，以 axp 为例
cd <codejoo>/apps/axp && npm run accept
```

## 接入新的客户端

1. 用你的语言写一个执行器：读取 `scenarios/*.json`，把中立的插件配置映射成你自己库的参数，把你的错误映射到 CONTRACT §7 的错误分类。
2. 采集 CONTRACT §7 列出的客户端事件。
3. 对每个网络预设，通过 netem 建一个代理，把 baseURL 指向它。
4. 缺少某项能力时，输出 `skipped: missing <cap>`，不算失败。
