#!/usr/bin/env node
// 场景校验脚本：不依赖 ajv，手写 scenario.schema.json 的关键规则。
// 用法：node tools/validate.mjs [scenarios 目录] [--check-index]   失败时退出码为 1。
// --check-index：额外检查 README.md 是否与 tools/gen-index.mjs 的生成结果一致。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildIndex } from './gen-index.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const checkIndex = args.includes('--check-index');
const dir = path.resolve(args.find((a) => !a.startsWith('--')) || path.join(here, '..', 'scenarios'));
const schema = JSON.parse(fs.readFileSync(path.join(here, '..', 'scenario.schema.json'), 'utf8'));
const defs = schema.$defs;

const CAPS = new Set(defs.capability.enum);
const NETS = new Set(defs.network.enum);
const STYLES = new Set(schema.properties.styles.items.enum);
const METHODS = new Set(defs.request.properties.method.enum);
const OPT_KEYS = new Set(Object.keys(defs.request.properties.options.properties));
const REQ_KEYS = new Set(Object.keys(defs.request.properties));
const STEP_KEYS = new Set(Object.keys(defs.step.properties));
const RESULT_KEYS = new Set(Object.keys(defs.result.properties));
const EXPECT_KEYS = new Set(Object.keys(defs.expectation.properties));
const SERVER_KEYS = new Set(Object.keys(defs.expectation.properties.server.properties));
const CLIENT_KEYS = new Set(Object.keys(defs.expectation.properties.client.properties));
const SCENARIO_KEYS = new Set(Object.keys(schema.properties));
const ERR_RE = new RegExp(defs.errorKind.pattern);
const ID_RE = new RegExp(schema.properties.id.pattern);
const EXPECT_KEY_RE = new RegExp(schema.properties.expect.propertyNames.pattern);
const SET_ACCESS = new Set(defs.step.properties.set_token.properties.access.enum);
const SET_REFRESH = new Set(defs.step.properties.set_token.properties.refresh.enum);

const errors = [];
const seenIds = new Map();
let count = 0;

/** 记录一条错误 */
const err = (where, msg) => errors.push(`${where}: ${msg}`);
const isObj = (v) => v && typeof v === 'object' && !Array.isArray(v);

/** 校验区间 {eq,min,max} */
function checkRange(where, r) {
  if (!isObj(r)) return err(where, '区间必须是对象');
  for (const [k, v] of Object.entries(r)) {
    if (!['eq', 'min', 'max'].includes(k)) err(where, `区间未知键 ${k}`);
    else if (!Number.isInteger(v)) err(where, `区间 ${k} 必须是整数`);
  }
  if (Object.keys(r).length === 0) err(where, '区间不能为空');
  if (r.min != null && r.max != null && r.min > r.max) err(where, 'min 大于 max');
}

/** 校验 multipart 请求体（schema $defs/multipartBody） */
function checkMultipart(where, body) {
  const extra = Object.keys(body).filter((k) => k !== 'multipart');
  if (extra.length) err(where, `multipart 请求体只能含 multipart 一个键，多出 ${extra.join(',')}`);
  const m = body.multipart;
  if (!isObj(m)) return err(where, 'multipart 必须是对象');
  for (const k of Object.keys(m)) if (!['fields', 'files'].includes(k)) err(where, `multipart 未知字段 ${k}`);
  if (m.fields != null) {
    if (!isObj(m.fields)) err(where, 'multipart.fields 必须是对象');
    else for (const [k, v] of Object.entries(m.fields)) if (typeof v !== 'string') err(where, `multipart.fields.${k} 必须是字符串`);
  }
  if (m.files != null) {
    if (!Array.isArray(m.files)) return err(where, 'multipart.files 必须是数组');
    m.files.forEach((f, i) => {
      const w = `${where}.multipart.files[${i}]`;
      if (!isObj(f)) return err(w, '文件必须是对象');
      for (const k of Object.keys(f)) if (!['name', 'filename', 'size', 'seed'].includes(k)) err(w, `未知字段 ${k}`);
      if (typeof f.name !== 'string' || !f.name) err(w, 'name 必须是非空字符串');
      if (typeof f.filename !== 'string') err(w, 'filename 必须是字符串');
      if (!Number.isInteger(f.size) || f.size < 0) err(w, 'size 必须是非负整数');
      if (!Number.isInteger(f.seed) || f.seed < 0 || f.seed > 4294967295) err(w, 'seed 必须是 0..4294967295 的整数');
    });
  }
}

/** 收集 step 中的请求 id；同时校验 step 结构 */
function walkStep(where, step, ids) {
  if (!isObj(step)) return err(where, 'step 必须是对象');
  const keys = Object.keys(step);
  if (keys.length !== 1) return err(where, `step 必须恰好一个键，实际 ${keys.length}：${keys.join(',')}`);
  const k = keys[0];
  const v = step[k];
  if (!STEP_KEYS.has(k)) return err(where, `未知 step 键 ${k}`);
  switch (k) {
    case 'request': {
      if (!isObj(v)) return err(where, 'request 必须是对象');
      for (const rk of Object.keys(v)) if (!REQ_KEYS.has(rk)) err(where, `request 未知字段 ${rk}`);
      if (typeof v.id !== 'string' || !v.id) err(where, 'request.id 缺失');
      if (typeof v.path !== 'string' || !v.path) err(where, 'request.path 缺失');
      if (v.method && !METHODS.has(v.method)) err(where, `非法 method ${v.method}`);
      if (v.query != null && !isObj(v.query)) err(where, 'query 必须是对象');
      if (v.options) {
        if (!isObj(v.options)) err(where, 'options 必须是对象');
        else for (const ok of Object.keys(v.options)) if (!OPT_KEYS.has(ok)) err(where, `options 未知字段 ${ok}`);
      }
      if (v.cancel_after_ms != null && !Number.isInteger(v.cancel_after_ms)) err(where, 'cancel_after_ms 必须是整数');
      if (isObj(v.body) && 'multipart' in v.body) checkMultipart(`${where}.body`, v.body);
      if (ids.has(v.id)) err(where, `请求 id 重复 ${v.id}`);
      ids.add(v.id);
      break;
    }
    case 'parallel':
    case 'sequence':
      if (!Array.isArray(v)) return err(where, `${k} 必须是数组`);
      v.forEach((s, i) => walkStep(`${where}.${k}[${i}]`, s, ids));
      break;
    case 'fire':
      walkStep(`${where}.fire`, v, ids);
      break;
    case 'repeat': {
      if (!isObj(v) || !Number.isInteger(v.times) || v.times < 1 || !v.step) return err(where, 'repeat 需要 times>=1 与 step');
      const inner = new Set();
      walkStep(`${where}.repeat`, v.step, inner);
      for (const id of inner) {
        if (v.id_suffix === false) ids.add(id);
        else for (let i = 0; i < v.times; i++) ids.add(`${id}#${i}`);
      }
      break;
    }
    case 'wait':
      if (!Number.isInteger(v) || v < 0) err(where, 'wait 必须是非负整数');
      break;
    case 'set_token':
      if (!isObj(v)) return err(where, 'set_token 必须是对象');
      if (v.access != null && !SET_ACCESS.has(v.access)) err(where, `set_token.access 非法 ${v.access}`);
      if (v.refresh != null && !SET_REFRESH.has(v.refresh)) err(where, `set_token.refresh 非法 ${v.refresh}`);
      break;
    case 'style':
      if (!STYLES.has(v)) err(where, `style 步骤取值非法 ${JSON.stringify(v)}`);
      break;
    case 'revoke':
      if (!isObj(v)) return err(where, 'revoke 必须是对象');
      if (v.scope != null && !['all', 'access'].includes(v.scope)) err(where, `revoke.scope 非法 ${v.scope}`);
      break;
    case 'network': {
      const okName = typeof v === 'string' && NETS.has(v);
      if (!okName && !isObj(v)) err(where, `network 必须是预设名或 profile 对象：${JSON.stringify(v)}`);
      break;
    }
    case 'login':
    case 'logout':
    case 'expire_local':
    case 'cancel_all':
    case 'clear_cache':
    case 'server_reset':
      if (!isObj(v)) err(where, `${k} 必须是对象`);
      break;
    default:
  }
}

/** 校验单条 result */
function checkResult(where, r, ids, selfId) {
  if (!isObj(r)) return err(where, 'result 必须是对象');
  for (const k of Object.keys(r)) if (!RESULT_KEYS.has(k)) err(where, `result 未知字段 ${k}`);
  if (r.ok != null && typeof r.ok !== 'boolean') err(where, 'ok 必须是布尔');
  if (r.error != null) {
    const list = Array.isArray(r.error) ? r.error : [r.error];
    if (list.length === 0) err(where, 'error 数组不能为空');
    for (const e of list) if (typeof e !== 'string' || !ERR_RE.test(e)) err(where, `非法错误类别 ${JSON.stringify(e)}`);
    if (r.ok === true) err(where, 'ok=true 不应带 error');
  }
  if (r.hit != null && !Number.isInteger(r.hit)) err(where, 'hit 必须是整数');
  if (r.from_cache != null && typeof r.from_cache !== 'boolean') err(where, 'from_cache 必须是布尔');
  if (r.same_as != null) {
    const s = r.same_as;
    if (!isObj(s) || typeof s.id !== 'string' || typeof s.field !== 'string' || !s.field) err(where, 'same_as 需要 {id, field}');
    else {
      for (const k of Object.keys(s)) if (!['id', 'field'].includes(k)) err(where, `same_as 未知字段 ${k}`);
      if (!ids.has(s.id)) err(where, `same_as 引用了不存在的请求 id: ${s.id}`);
      if (s.id === selfId) err(where, 'same_as 不能引用自己');
      if (r.ok !== true) err(where, 'same_as 只能用于 ok=true 的结果');
    }
  }
}

/** 校验一个 expectation */
function checkExpectation(where, e, ids) {
  if (!isObj(e)) return err(where, 'expectation 必须是对象');
  for (const k of Object.keys(e)) if (!EXPECT_KEYS.has(k)) err(where, `expectation 未知字段 ${k}`);
  if (e.skip != null && typeof e.skip !== 'string') err(where, 'skip 必须是字符串');
  if (e.results) {
    if (!isObj(e.results)) err(where, 'results 必须是对象');
    else for (const [id, r] of Object.entries(e.results)) {
      if (!ids.has(id)) err(where, `results 引用了不存在的请求 id: ${id}`);
      checkResult(`${where}.results.${id}`, r, ids, id);
    }
  }
  if (e.order) {
    if (!Array.isArray(e.order)) err(where, 'order 必须是数组');
    else for (const id of e.order) if (!ids.has(id)) err(where, `order 引用了不存在的请求 id: ${id}`);
  }
  if (e.server) {
    for (const [k, v] of Object.entries(e.server)) {
      if (!SERVER_KEYS.has(k)) { err(where, `server 未知字段 ${k}`); continue; }
      if (k === 'by_key' || k === 'by_path') for (const [kk, r] of Object.entries(v)) checkRange(`${where}.server.${k}.${kk}`, r);
      else checkRange(`${where}.server.${k}`, v);
    }
  }
  if (e.client) {
    for (const [k, v] of Object.entries(e.client)) {
      if (!CLIENT_KEYS.has(k)) { err(where, `client 未知字段 ${k}`); continue; }
      if (k === 'loading') {
        if (!Array.isArray(v) || v.some((x) => typeof x !== 'boolean')) err(where, 'client.loading 必须是布尔数组');
      } else checkRange(`${where}.client.${k}`, v);
    }
  }
  if (e.duration_ms) checkRange(`${where}.duration_ms`, e.duration_ms);
}

/** 校验单个场景 */
function checkScenario(file, sc, idx) {
  const where = `${file}[${idx}]${sc && sc.id ? ' ' + sc.id : ''}`;
  if (!isObj(sc)) return err(where, '场景必须是对象');
  for (const k of ['id', 'title', 'requires', 'networks', 'steps', 'expect']) if (!(k in sc)) err(where, `缺少必填字段 ${k}`);
  for (const k of Object.keys(sc)) if (!SCENARIO_KEYS.has(k)) err(where, `未知字段 ${k}`);
  if (typeof sc.id !== 'string' || !ID_RE.test(sc.id)) err(where, `id 不符合格式 ${sc.id}`);
  else if (seenIds.has(sc.id)) err(where, `id 重复（另见 ${seenIds.get(sc.id)}）`);
  else seenIds.set(sc.id, file);
  if (typeof sc.title !== 'string' || !sc.title) err(where, 'title 必须是非空字符串');
  if (typeof sc.why !== 'string' || !sc.why) err(where, '缺少 why（防什么真实问题）');
  if (!Array.isArray(sc.requires)) err(where, 'requires 必须是数组');
  else for (const c of sc.requires) if (!CAPS.has(c)) err(where, `requires 含未知能力 ${c}`);
  if (!Array.isArray(sc.networks) || sc.networks.length === 0) err(where, 'networks 不能为空');
  else for (const n of sc.networks) if (!NETS.has(n)) err(where, `未知网络预设 ${n}`);
  const styles = sc.styles ?? [...STYLES];
  if (sc.styles && (!Array.isArray(sc.styles) || sc.styles.length === 0 || sc.styles.some((s) => !STYLES.has(s)))) err(where, 'styles 非法');
  if (sc.timeout_ms != null && !Number.isInteger(sc.timeout_ms)) err(where, 'timeout_ms 必须是整数');
  if (sc.client) {
    if (!isObj(sc.client)) err(where, 'client 必须是对象');
    else {
      for (const k of Object.keys(sc.client)) if (!['base_timeout_ms', 'plugins'].includes(k)) err(where, `client 未知字段 ${k}`);
      if (sc.client.plugins) for (const [p, o] of Object.entries(sc.client.plugins)) {
        if (!CAPS.has(p)) err(where, `client.plugins 含未知能力 ${p}`);
        if (!isObj(o)) err(where, `client.plugins.${p} 必须是对象`);
        else if (!sc.requires.includes(p) && p !== 'log') err(where, `插件 ${p} 已安装但 requires 未声明`);
      }
    }
  }
  const ids = new Set();
  if (!Array.isArray(sc.steps) || sc.steps.length === 0) err(where, 'steps 不能为空');
  else sc.steps.forEach((s, i) => walkStep(`${where}.steps[${i}]`, s, ids));

  // 新能力名的一致性：用到对应特性就必须在 requires 里声明，否则缺能力的执行器不会跳过
  if (Array.isArray(sc.requires)) {
    const usesJson = JSON.stringify(sc.steps ?? []);
    const plugins = sc.client?.plugins ?? {};
    const need = [
      ['multipart', /"multipart":s*{/.test(usesJson)],
      ['share-retry', plugins.share?.policy === 'retry' || /"share":s*"retry"/.test(usesJson)],
      ['timeout-send', plugins.timeout?.send_ms != null],
    ];
    for (const [cap, used] of need) if (used && !sc.requires.includes(cap)) err(where, `用到了 ${cap} 相关特性但 requires 未声明 ${cap}`);
  }

  if (!isObj(sc.expect)) return err(where, 'expect 必须是对象');
  for (const [key, e] of Object.entries(sc.expect)) {
    if (!EXPECT_KEY_RE.test(key)) { err(where, `expect 键非法 ${key}`); continue; }
    const [net, st] = key.split('@');
    if (net !== '*' && Array.isArray(sc.networks) && !sc.networks.includes(net)) err(where, `expect 键 ${key} 的网络不在 networks 中`);
    if (st && !styles.includes(st)) err(where, `expect 键 ${key} 的风格不在 styles 中`);
    checkExpectation(`${where}.expect[${key}]`, e, ids);
  }
  // 覆盖检查：每个 (网络, 风格) 组合至少命中 网络@风格 / 网络 / *@风格 / * 之一
  if (Array.isArray(sc.networks)) for (const n of sc.networks) for (const st of styles) {
    const hit = [`${n}@${st}`, n, `*@${st}`, '*'].some((k) => k in sc.expect);
    if (!hit) err(where, `expect 未覆盖 网络=${n} 风格=${st}`);
  }
  // 语义自检：failAll 型预期（全部 network/timeout）不应与精确服务端计数同时出现于 total>0
  count++;
}

if (!fs.existsSync(dir)) { console.error('目录不存在: ' + dir); process.exit(1); }
const files = fs.readdirSync(dir).filter((f) => f.endsWith('.json')).sort();
for (const f of files) {
  let data;
  try { data = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8')); } catch (e) { err(f, `JSON 解析失败: ${e.message}`); continue; }
  if (!Array.isArray(data)) { err(f, '顶层必须是数组'); continue; }
  data.forEach((sc, i) => checkScenario(f, sc, i));
}

if (errors.length) {
  console.error(`校验失败：${errors.length} 个问题`);
  for (const e of errors.slice(0, 200)) console.error('  - ' + e);
  process.exit(1);
}
if (checkIndex) {
  const readme = path.join(dir, 'README.md');
  const cur = fs.existsSync(readme) ? fs.readFileSync(readme, 'utf8') : '';
  if (cur !== buildIndex(dir)) {
    console.error('README.md 已过期：请运行 node tools/gen-index.mjs 重新生成');
    process.exit(1);
  }
  console.log('索引检查通过');
}
console.log(`校验通过：${files.length} 个文件，${count} 个场景，id 全局唯一`);
