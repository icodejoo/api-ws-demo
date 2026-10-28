#!/usr/bin/env node
// 由 scenarios/*.json 生成 scenarios/README.md（零依赖）。
// 用法：node tools/gen-index.mjs [scenarios 目录]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_DIR = path.join(here, '..', 'scenarios');
const NETS = ['normal', 'weak', 'lossy', 'offline', 'blackhole', 'flap', 'dns'];
const STYLES = ['http', 'business'];
const LABELS = {
  basic: '基础', auth: '鉴权', retry: '重试', cache: '缓存', share: '请求合并', concurrency: '并发',
  cancel: '取消', loading: '加载状态', 'notify-normalize': '通知与信封', faults: '故障响应',
  network: '网络条件', timeout: '超时（dioman）', breaker: '熔断（dioman）', offline: '离线（dioman）',
  'repath-key-filter': '路径/键/过滤', combos: '组合交互', mock: '模拟',
};
const ORDER = Object.keys(LABELS);

/** 约定说明（固定文案） */
const CONVENTIONS = `## 约定（执行器必读）

- **每个场景 × 每个声明的网络 × 每种风格**各跑一遍（风格默认 http、business 都跑，见 CONTRACT §2.1；\`styles\` 可收窄）。
  预期里的错误优先用 \`status:<n>\`（与风格无关）。需要区分风格时才用 \`<网络>@business\` 键。
- **business 风格要求客户端把"HTTP 200 + code=n"也识别成逻辑状态 n**：涉及 status/fail_times/fail_every/barrier fail 的场景已自动配上 \`normalize\` 插件并在 \`requires\` 里声明。
  retry / breaker / notify 等对失败的判定，要求按**逻辑状态**（两种风格一致）生效，这是本目录有意为之的要求。
- **expect 没用 '*'**：每个网络都显式写出，避免 results/server/client 浅合并时 weak 与 offline 的 server 断言互相污染。weak/lossy/flap 不含 \`order\`/\`duration_ms\`。
- **weak/lossy/flap 只断言最终结果与资源释放**；offline/dns/blackhole 下无鉴权的场景 \`server.total eq 0\`。
- **login/logout/revoke/server_reset 走旁路**（CONTRACT 附录 B.2）：不经过 netem 和插件链，所以含 login 的场景也可以跑 offline/dns/blackhole，被测请求应得到 network/timeout，**不能误判 expired，也不能清掉 token**；这类场景用 \`by_path\`（旁路登录会计入服务端 total）。
- **revoke 默认 scope=all**（access+refresh 都吊销，应判 expired）；要测"刷新后恢复"必须显式写 \`scope:"access"\`。
- **\`style\` 步骤**在运行中切换响应风格，用到它的场景 \`styles\` 只写 \`http\`。
- **\`/t/mock/*\`**：mock 插件的基址固定为 \`<baseURL>/t/mock\`，场景用 \`data.mock=true\` 与真实路径 by_path 为 0 判断路由。
- **status 与 code 同时给出**（附录 B.1）：http 风格归 \`status:<n>\`，business 风格归 \`business:<code>\`，场景用 \`@business\` 键区分（\`faults.status-with-business\`、\`normalize.http-error-wins\`）。
- **blackhole** 场景带 \`client.base_timeout_ms=4000\`；weak/blackhole/flap/lossy 场景 \`timeout_ms=40000\`。
- **server 计数**：\`by_path\` 的 key 是请求路径，缺失按 0 计；登录、刷新也计入 \`by_path\`（所以 auth 场景用 by_path 而不用 total）。
- **\`from_cache: true\`** = 服务端日志里没有该请求。
- **repeat 的内部 id** 为 \`<id>#<i>\`，i 从 0 开始。
- **\`data\` 子集匹配**：客户端最终拿到的是信封里的 \`data\`（CONTRACT 附录 A.1，\`data.request.body.json\` 为解析后的请求体）。
- **并发 32/64 的场景假设执行器没有每主机连接数限制**（浏览器环境下要放宽或跳过）。
- 中立插件选项扩展：\`auth.refresh_query\`、\`key.scope='user'\`、\`retry.methods\`、\`cache.methods\`/\`cache.max\`（见 schema pluginOptions）。映射不了必须显式报错。
- **细分能力与新字段**（CONTRACT §7）：\`multipart\`（\`body.multipart\`，文件内容按 seed 的 LCG 算法生成）、\`keep-alive\`、\`share-retry\`、\`timeout-send\`（\`timeout.send_ms\`）、\`mock-in-chain\`；缺这些能力的执行器按"缺能力"跳过。结果项 \`same_as\` 要求两个成功请求 data 的同一字段相等（如 \`meta.conn\`）。`;

/**
 * 读取目录下全部场景并生成 README 文本。
 * @param {string} dir scenarios 目录
 * @returns {string} README 内容
 */
export function buildIndex(dir = DEFAULT_DIR) {
  const names = fs.readdirSync(dir).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5));
  names.sort((a, b) => {
    const ia = ORDER.indexOf(a); const ib = ORDER.indexOf(b);
    return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.localeCompare(b);
  });
  const files = names.map((n) => ({ name: n, label: LABELS[n] || n, list: JSON.parse(fs.readFileSync(path.join(dir, n + '.json'), 'utf8')) }));
  const total = files.reduce((a, f) => a + f.list.length, 0);
  const stylesOf = (s) => s.styles || STYLES;

  const matrix = files.map((f) => {
    const cells = NETS.map((n) => f.list.filter((s) => s.networks.includes(n)).length);
    return `| ${f.label}（${f.name}.json） | ${f.list.length} | ${cells.join(' | ')} |`;
  });
  const sumNet = NETS.map((n) => files.reduce((a, f) => a + f.list.filter((s) => s.networks.includes(n)).length, 0));

  const styleRows = files.map((f) => {
    const both = f.list.filter((s) => stylesOf(s).length === 2).length;
    const only = (st) => f.list.filter((s) => stylesOf(s).length === 1 && stylesOf(s)[0] === st).length;
    const biz = f.list.filter((s) => Object.keys(s.expect).some((k) => k.endsWith('@business'))).length;
    return `| ${f.label} | ${both} | ${only('http')} | ${only('business')} | ${biz} |`;
  });
  const allS = files.flatMap((f) => f.list);
  const sumBoth = allS.filter((s) => stylesOf(s).length === 2).length;
  const sumHttp = allS.filter((s) => stylesOf(s).length === 1 && stylesOf(s)[0] === 'http').length;
  const sumBiz = allS.filter((s) => stylesOf(s).length === 1 && stylesOf(s)[0] === 'business').length;
  const sumOverride = allS.filter((s) => Object.keys(s.expect).some((k) => k.endsWith('@business'))).length;
  const runs = allS.reduce((a, s) => a + s.networks.length * stylesOf(s).length, 0);

  const index = files.map((f) => `### ${f.label}（${f.name}.json，${f.list.length} 个）\n\n`
    + f.list.map((s) => `- \`${s.id}\` [${s.networks.join(',')}${s.styles ? ' | ' + s.styles.join(',') : ''}] ${s.title}`).join('\n'));

  return `<!-- 由 tools/gen-index.mjs 生成，勿手改。改完 scenarios/*.json 后运行：node tools/gen-index.mjs -->
# 场景目录

> 本文件由 \`tools/gen-index.mjs\` 生成，**勿手改**。\`node tools/validate.mjs --check-index\` 会检查它是否过期。

与语言无关的客户端验收场景，格式见 \`../scenario.schema.json\`，契约见 \`../CONTRACT.md\`。axp（TS/axios）与 dioman（Dart/dio）共用同一份。
校验：\`node tools/validate.mjs\`（合法 JSON、schema 关键规则、id 唯一、expect 覆盖 networks 与 styles、results 引用的 id 存在）。

共 **${total}** 个场景，${files.length} 个文件，每个文件顶层是场景数组；按（网络 × 风格）展开共 **${runs}** 次运行。

${CONVENTIONS}

## 覆盖矩阵（场景数：领域 × 网络预设）

| 领域 | 合计 | ${NETS.join(' | ')} |
|---|---|${NETS.map(() => '---').join('|')}|
${matrix.join('\n')}
| **合计** | ${total} | ${sumNet.join(' | ')} |

（一个场景可声明多个网络，所以每行网络列之和大于合计。）

## 风格统计（CONTRACT §2.1）

| 领域 | 两种风格都跑 | 仅 http | 仅 business | 含 \`@business\` 专属预期 |
|---|---|---|---|---|
${styleRows.join('\n')}
| **合计** | ${sumBoth} | ${sumHttp} | ${sumBiz} | ${sumOverride} |

## 场景索引

方括号内依次是网络预设，竖线后是收窄的风格（缺省表示两种都跑）。

${index.join('\n\n')}
`;
}

// 直接运行时写文件
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const dir = path.resolve(process.argv[2] || DEFAULT_DIR);
  const out = buildIndex(dir);
  fs.writeFileSync(path.join(dir, 'README.md'), out);
  console.log(`已生成 ${path.join(dir, 'README.md')}`);
}
