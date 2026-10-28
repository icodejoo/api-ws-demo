#!/usr/bin/env node
// 由人工维护的 coverage/axp-api.json 与 scenarios/*.json 生成 COVERAGE.md（零依赖）。
// 用法：node tools/gen-coverage.mjs [scenarios 目录] [--check]
// --check 只比较生成结果；文件缺失或过期时退出码为 1。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(here, '..');
const DEFAULT_DIR = path.join(ROOT, 'scenarios');
const CATALOG = path.join(ROOT, 'coverage', 'axp-api.json');
const OUTPUT = path.join(ROOT, 'COVERAGE.md');
const KINDS = new Set(['plugin', 'pluginOther', 'request', 'step', 'eventPositive', 'eventExists', 'installed', 'anyInstalled', 'client', 'method', 'protectedSuccess', 'retryAfter']);
const own = (o, k) => o != null && Object.hasOwn(o, k);
const get = (o, key) => key.split('.').reduce((v, k) => v == null ? undefined : v[k], o);
const capability = (p) => p === 'logger' ? 'log' : p;
const escape = (v) => String(v ?? '').replace(/\|/g, '&#124;').replace(/[\r\n]+/g, ' ');
const code = (v) => '`' + escape(v).replace(/`/g, '&#96;') + '`';
const pct = (n, d) => d ? (100 * n / d).toFixed(1) + '%' : '—';

/** 只遍历契约中的 step，不把 body/query 中的同名字段误判为步骤。 */
function walkSteps(steps) {
  return steps.flatMap((s) => {
    if (s.parallel) return [s, ...walkSteps(s.parallel)];
    if (s.sequence) return [s, ...walkSteps(s.sequence)];
    if (s.fire) return [s, ...walkSteps([s.fire])];
    if (s.repeat) return [s, ...walkSteps([s.repeat.step])];
    return [s];
  });
}

/** 校验人工台账，拼错规则或 API 引用直接失败，避免静默降低统计质量。 */
function validateCatalog(c) {
  if (c.plugins.length !== 14 || new Set(c.plugins).size !== 14) throw new Error('插件列表必须为 14 个且不重复');
  const ids = new Set();
  for (const a of c.apis) {
    if (ids.has(a.id)) throw new Error('API id 重复：' + a.id);
    ids.add(a.id);
    if (![...c.plugins, 'public'].includes(a.plugin) || !a.id.startsWith(a.plugin + '.')) throw new Error('API 插件不符：' + a.id);
    if (!a.category || !a.description || !a.source || typeof a.expressible !== 'boolean' || !a.expression || !Array.isArray(a.rules)) throw new Error('API 字段缺失：' + a.id);
    if (!['public', 'internal'].includes(a.scope)) throw new Error('API 范围缺失或错误：' + a.id);
    for (const r of a.rules) {
      if (!KINDS.has(r.kind)) throw new Error('未知规则：' + r.kind);
      if (r.confidence && r.confidence !== 'doubt') throw new Error('未知置信度：' + a.id);
      if (['plugin', 'pluginOther', 'request', 'step', 'eventPositive', 'eventExists'].includes(r.kind) && !r.path) throw new Error('规则缺少路径：' + a.id);
      if (r.kind === 'method' && typeof r.method !== 'string') throw new Error('规则缺少 method：' + a.id);
      if (r.kind === 'pluginOther' && !c.plugins.includes(r.plugin)) throw new Error('规则引用未知插件：' + a.id);
    }
    if (!a.expressible && a.rules.length) throw new Error('无法表达的 API 不应有命中规则：' + a.id);
  }
  for (const a of c.apis) if (a.variantOf && !ids.has(a.variantOf)) throw new Error('未知 variantOf：' + a.id);
  for (const e of c.publicExports) if (e.api && !ids.has(e.api)) throw new Error('入口导出未登记：' + e.name);
  for (const r of c.risks) if (r.plugins.length !== 2 || r.plugins.some(p => !c.plugins.includes(p))) throw new Error('组合风险引用错误');
}

/** 选项按实际映射位置匹配；显式 false/0 也算字段触达，不依赖 truthy。 */
function matches(rule, a, ctx) {
  const { s, requests, steps, expectations } = ctx;
  const p = capability(a.plugin);
  const selected = (v) => v !== undefined && (!own(rule, 'equals') || v === rule.equals);
  switch (rule.kind) {
    case 'installed': return own(s.client?.plugins, p);
    case 'anyInstalled': return ctx.installed.size > 0;
    case 'client': return requests.length > 0;
    case 'method': return requests.some(r => (r.method ?? 'GET').toUpperCase() === rule.method);
    case 'plugin': return selected(get(s.client?.plugins?.[p], rule.path));
    case 'pluginOther': return selected(get(s.client?.plugins?.[capability(rule.plugin)], rule.path));
    case 'request': return requests.some(r => selected(get(r.options, rule.path)));
    case 'step': return steps.some(s => own(s, rule.path));
    case 'eventExists': return expectations.some(e => own(e.client, rule.path));
    case 'eventPositive': return expectations.some(e => {
      const v = e.client?.[rule.path];
      return v?.eq > 0 || v?.min > 0;
    });
    case 'protectedSuccess': return requests.some(r => {
      if (r.options?.auth === false) return false;
      const prefix = s.client?.plugins?.auth?.url_pattern ?? '/t/private';
      if (r.options?.auth !== true && !r.path.startsWith(prefix)) return false;
      return expectations.some(e => Object.entries(e.results ?? {}).some(([id, result]) => (id === r.id || id.startsWith(r.id + '#')) && result.ok === true));
    });
    case 'retryAfter': return requests.some(r => {
      const retry = r.options?.retry;
      if (retry === false) return false;
      const cfg = typeof retry === 'object' ? retry : {};
      const max = typeof retry === 'number' ? retry : cfg.max ?? s.client?.plugins?.retry?.max ?? 0;
      const respect = cfg.respect_retry_after ?? s.client?.plugins?.retry?.respect_retry_after ?? true;
      const status = Number(r.query?.fail_status ?? r.query?.status);
      return max > 0 && respect && own(r.query, 'retry_after') && [413, 429, 503].includes(status)
        && expectations.some(e => e.duration_ms || e.server?.total || e.server?.by_path);
    });
    default: throw new Error('未知规则：' + rule.kind);
  }
}

/** 读取全部场景并生成稳定文本；不读取验收结果，不推断通过率。 */
export function buildCoverage(dir = DEFAULT_DIR, catalogFile = CATALOG) {
  const c = JSON.parse(fs.readFileSync(catalogFile, 'utf8'));
  validateCatalog(c);
  const supported = new Set(c.plugins.map(capability));
  const seen = new Set();
  const contexts = fs.readdirSync(dir).filter(f => f.endsWith('.json')).sort().flatMap(file => {
    const list = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
    if (!Array.isArray(list)) throw new Error(file + ' 顶层必须是数组');
    return list.map(s => {
      if (seen.has(s.id)) throw new Error('场景 id 重复：' + s.id);
      seen.add(s.id);
      const steps = walkSteps(s.steps);
      const requests = steps.filter(s => s.request).map(s => s.request);
      const installed = new Set(c.plugins.filter(p => own(s.client?.plugins, capability(p))));
      const unsupported = [...new Set([...(s.requires ?? []), ...Object.keys(s.client?.plugins ?? {})])].filter(p => !supported.has(p));
      const expectations = Object.values(s.expect).filter(e => !e.skip);
      return { s, file, steps, requests, installed, expectations, unsupported, eligible: !unsupported.length && expectations.length > 0 && requests.length > 0 };
    });
  });
  const refs = (ctx) => `[${code(ctx.s.id)}](scenarios/${ctx.file})`;
  const refsOf = (contexts) => contexts.map(refs).join('、');
  const internal = c.apis.filter(a => a.scope === 'internal');
  const rows = c.apis.filter(a => a.scope !== 'internal').map(a => {
    const covered = [], doubt = [], skipped = [];
    for (const ctx of contexts) {
      if (a.plugin !== 'public' && !ctx.installed.has(a.plugin)) continue;
      const rules = a.rules.filter(r => matches(r, a, ctx));
      if (!rules.length) continue;
      if (!ctx.eligible) skipped.push(ctx);
      else if (rules.some(r => r.confidence !== 'doubt')) covered.push(ctx);
      else doubt.push(ctx);
    }
    return { a, covered, doubt, skipped, single: covered.filter(ctx => ctx.installed.size === 1 && a.plugin !== 'public') };
  });
  const pluginRows = rows.filter(r => r.a.plugin !== 'public');
  const gaps = pluginRows.filter(r => !r.covered.length);
  const singles = pluginRows.filter(r => !r.single.length);
  const pairRows = [];
  for (let i = 0; i < c.plugins.length; i++) for (let j = i + 1; j < c.plugins.length; j++) {
    const a = c.plugins[i], b = c.plugins[j];
    // 安装或 requires 声明均算“涉及”；同时给出真正可在 axp 执行的数量。
    const involving = ctx => [a, b].every(p => ctx.installed.has(p) || ctx.s.requires?.includes(capability(p)));
    const declared = contexts.filter(involving);
    const eligible = declared.filter(ctx => ctx.eligible && ctx.installed.has(a) && ctx.installed.has(b));
    const risk = c.risks.find(r => r.plugins.includes(a) && r.plugins.includes(b));
    pairRows.push({ a, b, declared, eligible, risk });
  }
  const pairGaps = pairRows.filter(r => !r.eligible.length);
  const pair = (a, b) => pairRows.find(r => r.a === a && r.b === b);
  const md = [
    '<!-- 由 tools/gen-coverage.mjs 生成，勿手改。 -->',
    '# axp API 覆盖反查台账', '',
    '由 ' + code('coverage/axp-api.json') + ' 与 ' + code('scenarios/*.json') + ' 生成。更新：' + code('node testkit/tools/gen-coverage.mjs') + '；检查：' + code('node testkit/tools/gen-coverage.mjs --check') + '。', '',
    ...c.notes.map(n => '- ' + n), '',
    '统计口径：已覆盖 = 可在 axp 执行的场景具有明确配置/步骤/事件断言证据；存疑不计覆盖。字段触达允许只覆盖某种形式，表达列注明限制，不能解读成所有值域、回调异常或分支都覆盖。安装只证明工厂/install 调用，不证明 enable 等选项。', '',
    '“单插件缺口”表示没有严格单插件证据；组合场景中的证据仍列在明细。组合矩阵只说明共同声明/安装，不证明有交互断言。', '',
    `扫描 **${contexts.length}** 个场景；可执行 **${contexts.filter(x => x.eligible).length}** 个。14 插件 API **${pluginRows.length}** 项，已覆盖 **${pluginRows.length - gaps.length}** 项，API 缺口 **${gaps.length}** 项（含存疑 **${gaps.filter(r => r.doubt.length).length}** 项）；严格单插件缺口 **${singles.length}** 项。可执行组合缺口 **${pairGaps.length}/91**，其中有明确交互风险 **${pairGaps.filter(r => r.risk).length}** 对。`, '',
    '## 每插件汇总', '',
    '| 插件 | API 总数 | 已覆盖（任意组合） | 覆盖率 | API 缺口 | 严格单插件已覆盖 | 单插件缺口 |',
    '|---|---:|---:|---:|---:|---:|---:|',
  ];
  for (const p of [...c.plugins, 'public']) {
    const list = rows.filter(r => r.a.plugin === p), hit = list.filter(r => r.covered.length).length, single = list.filter(r => r.single.length).length;
    md.push(`| ${p === 'public' ? '入口/装配（另计）' : p} | ${list.length} | ${hit} | ${pct(hit, list.length)} | ${list.length-hit} | ${p === 'public' ? '—' : single} | ${p === 'public' ? '—' : list.length-single} |`);
  }
  md.push('', '## 逐 API 明细', '', '类别：opt 选项、cb 回调、req 请求配置、global 全局配置、fn 导出函数、const 常量、method 生命周期、prop 属性、export/handle/member 入口及公共成员。源码位置相对 axp 根目录。JSDoc 明确 @internal 的导出另列，不计外部验收目标；index 明确重导出的符号仍按公开 API 计。', '');
  for (const p of [...c.plugins, 'public']) {
    md.push('### ' + p, '', '| API id | 说明 / 源码 | 中立表达 / 证据规则 | 覆盖场景（完整列表） | 严格单插件场景 |', '|---|---|---|---|---|');
    for (const r of rows.filter(r => r.a.plugin === p)) {
      const { a } = r;
      let evidence = r.covered.length ? refsOf(r.covered) : '**缺口**';
      if (r.doubt.length) evidence += '；存疑：' + refsOf(r.doubt);
      if (r.skipped.length) evidence += '；不可执行候选：' + refsOf(r.skipped);
      md.push(`| ${code(a.id)} | ${escape(a.description)}；${code(a.source)}${a.internal ? '（@internal）' : ''} | ${a.expressible ? '' : '**中立 schema 无法表达**；'}${escape(a.expression)} | ${evidence} | ${p === 'public' ? '—' : r.single.length ? refsOf(r.single) : '**缺口**'} |`);
    }
    md.push('');
  }
  md.push('## 内部导出盘点（不计公开 API 分母）', '', `共 ${internal.length} 项，JSDoc 明确 @internal，用于单测或内部实现；不把中立 schema 无法直接调用内部函数当成产品 API 缺口。`, '', '| id | 源码 | 说明 |', '|---|---|---|');
  for (const a of internal) md.push(`| ${code(a.id)} | ${code(a.source)} | ${escape(a.description)} |`);
  md.push('', '## src/index.ts 公开导出', '', '别名指向同一 API，不重复计数。类型导出全部列出；JSON 场景无法验证类型契约，需另设 TS 编译验收。', '', '| 导出名 | 来源 | 类型/运行时 | API 对应 |', '|---|---|---|---|');
  for (const e of c.publicExports) md.push(`| ${code(e.name)} | ${code(e.module)}（${code(e.source)}） | ${e.typeOnly ? '仅类型：编译验收缺口' : '运行时'} | ${e.api ? code(e.api) : escape(e.note)} |`);
  md.push('', '## 插件两两组合矩阵', '', '14×14 上三角。每格为“涉及场景数 / axp 可执行且共同安装场景数”，覆盖 combos.json 及所有其它场景；同一场景只计一次，不按网络/风格倍增。**0** 显式标出，后一数为 0 时仍是组合缺口。', '', '| 插件 | ' + c.plugins.join(' | ') + ' |', '|---|' + c.plugins.map(() => '---:').join('|') + '|');
  for (let i = 0; i < c.plugins.length; i++) {
    const cells = c.plugins.map((p, j) => {
      if (j <= i) return j === i ? '—' : '';
      const r = pair(c.plugins[i], p);
      return `${r.declared.length || '**0**'} / ${r.eligible.length || '**0**'}`;
    });
    md.push('| ' + c.plugins[i] + ' | ' + cells.join(' | ') + ' |');
  }
  md.push('', '### 组合证据反查', '', '| 组合 | 涉及数 / 可执行数 | 可执行场景 | 仅声明/不可执行场景 |', '|---|---:|---|---|');
  for (const r of pairRows) md.push(`| ${r.a} + ${r.b} | ${r.declared.length} / ${r.eligible.length} | ${refsOf(r.eligible) || '**0**'} | ${refsOf(r.declared.filter(x => !r.eligible.includes(x))) || '—'} |`);
  md.push('', '## 按优先级排序的缺口', '', '### 先补单插件 API', '', 'P1 为高风险行为选项；P2 为其它字段、生命周期和导出。以下包括“只有组合证据”的 API，已覆盖不等于完成单插件阶段。无法表达的项需先扩展中立契约/映射；本次只登记。', '', '| 优先级 | API | 缺口原因 |', '|---|---|---|');
  const sortGap = (x, y) => x.a.priority-y.a.priority || (x.a.rank ?? 999)-(y.a.rank ?? 999) || x.a.id.localeCompare(y.a.id, 'en');
  for (const r of [...singles].sort(sortGap)) md.push(`| P${r.a.priority} | ${code(r.a.id)} | ${r.covered.length ? '仅组合覆盖，缺独立场景' : r.doubt.length ? '存疑，未计覆盖' : '无有效场景'}；${escape(r.a.expression)} |`);
  const publicGaps = rows.filter(r => r.a.plugin === 'public' && !r.covered.length);
  md.push('', '### 入口及装配缺口（另计）', '', ...publicGaps.map(r => '- ' + code(r.a.id) + '：' + escape(r.a.expression) + (r.doubt.length ? '；存疑' : '')), '', '### 再补有交互风险的组合缺口', '', '| 优先级 | 组合 | 涉及数 / 可执行数 | 风险 |', '|---|---|---:|---|');
  for (const r of pairGaps.filter(r => r.risk).sort((x,y) => x.risk.priority-y.risk.priority || (x.a+x.b).localeCompare(y.a+y.b, 'en'))) md.push(`| P${r.risk.priority + 2} | ${r.a} + ${r.b} | ${r.declared.length} / 0 | ${escape(r.risk.risk)} |`);
  md.push('', '其它可执行组合缺口：' + (pairGaps.filter(r => !r.risk).map(r => code(r.a + '+' + r.b)).join('、') || '无') + '。', '', '### 已有共同安装但尚需核实交互断言', '', '以下只列风险，不计作“零场景缺口”；共同安装数不能证明隔离、重放次数、清理或异常分支。', '', ...pairRows.filter(r => r.risk && r.eligible.length).map(r => '- ' + code(r.a + '+' + r.b) + '：' + escape(r.risk.risk)), '', '## 维护方式', '', '1. axp 类型/JSDoc 变化时人工核对并更新 API 清单（id 保持稳定，源码位置及 declaration 留作依据）。', '2. 每条映射规则只按明确字段、step 或事件断言匹配；没有证据就留空 rules，拿不准用 confidence=doubt。', '3. 执行生成、--check 及 tools/validate.mjs --check-index；--check 不写文件，任何差异均非零退出。', '');
  return md.join('\n');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2);
    if (args.some(a => a.startsWith('--') && a !== '--check') || args.filter(a => !a.startsWith('--')).length > 1) throw new Error('用法：node tools/gen-coverage.mjs [scenarios 目录] [--check]');
    const dir = path.resolve(args.find(a => !a.startsWith('--')) || DEFAULT_DIR);
    const out = buildCoverage(dir);
    if (args.includes('--check')) {
      const current = fs.existsSync(OUTPUT) ? fs.readFileSync(OUTPUT, 'utf8') : '';
      if (current !== out) throw new Error('COVERAGE.md 已过期：请运行 node testkit/tools/gen-coverage.mjs');
      console.log('覆盖台账检查通过');
    } else {
      fs.writeFileSync(OUTPUT, out);
      console.log('已生成 ' + OUTPUT);
    }
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
  }
}

