import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const here = path.dirname(fileURLToPath(import.meta.url));
function validate(steps, requires = [], client, expect = { '*': {} }) {
  const dir = fs.mkdtempSync(path.join(here, '.validate-tests-'));
  try {
    fs.writeFileSync(path.join(dir, 'sample.json'), JSON.stringify([{
      id: 'sample.controls', title: 'sample', why: '验证控制字段与普通请求体隔离',
      requires, networks: ['normal'], styles: ['http'], steps,
      expect, ...(client ? { client } : {}),
    }]));
    return spawnSync(process.execPath, [path.join(here, 'validate.mjs'), dir], { encoding: 'utf8' });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}
const request = (extra = {}) => ({ request: { id: 'r', path: '/t/echo', ...extra } });

test('普通 JSON 同名字段不触发取消或 share-retry 能力', () => {
  const result = validate([request({ body: {
    cancel_after_ms: 10, cancel_all: false, share: 'retry',
    nested: { request: { id: 'fake', query: { code: 1001 } } },
  } })]);
  assert.equal(result.status, 0, result.stderr);
});
test('嵌套实际取消控制缺 cancel 时拒绝，有声明时接受', () => {
  const steps = [{ sequence: [{ parallel: [{ fire: { repeat: { times: 1, step: request({ cancel_after_ms: 10 }) } } }] }] }];
  assert.match(validate(steps).stderr, /requires 未声明 cancel/);
  assert.equal(validate(steps, ['cancel']).status, 0);
});
test('cancel_all step 缺 cancel 时拒绝', () => {
  assert.match(validate([{ cancel_all: {} }]).stderr, /requires 未声明 cancel/);
  assert.equal(validate([{ cancel_all: {} }], ['cancel']).status, 0);
});
test('total_ms 必须声明 timeout-total', () => {
  const client = { plugins: { timeout: { total_ms: 20 } } };
  assert.match(validate([request()], ['timeout'], client).stderr, /requires 未声明 timeout-total/);
  assert.equal(validate([request()], ['timeout', 'timeout-total'], client).status, 0);
});

test('repeat 展开 id 后仍要求 raw-business-code，id_suffix=false 保持原 id', () => {
  for (const suffix of [true, false]) {
    const steps = [{ repeat: { times: suffix ? 2 : 1, id_suffix: suffix, step: request({ query: { code: 5001 } }) } }];
    const results = suffix ? { 'r#0': { ok: true }, 'r#1': { ok: true } } : { r: { ok: true } };
    const expect = { '*': { results } };
    assert.match(validate(steps, [], undefined, expect).stderr, /requires 未声明 raw-business-code/);
    assert.equal(validate(steps, ['raw-business-code'], undefined, expect).status, 0);
  }
});
