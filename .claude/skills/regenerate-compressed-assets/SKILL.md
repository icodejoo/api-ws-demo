---
name: regenerate-compressed-assets
description: Regenerate the 5 static pre-compressed test assets (gzip/zstd × JSON/msgpack) under assets/ after changing assets/compressed_sample.json. Use whenever that source file is edited, since the derived .gz/.zst/.msgpack* files are committed binaries that must stay in sync with it — the Rust server embeds them via include_bytes! and never compresses anything at request time.
---

# Regenerate compressed test assets

`assets/compressed_sample.json` is the single source of truth. The other 5 files in `assets/` are
derived, committed binaries — they are **not** generated at build time or request time (that's the
whole point: zero runtime CPU cost regardless of traffic, per `CLAUDE.md`'s "static test assets"
constraint). Whenever `compressed_sample.json` changes, all 5 derived files must be regenerated and
committed alongside it, or the HTTP `/api/compressed*` endpoints and the matching STOMP topics will
serve stale data.

## Steps

1. Edit `assets/compressed_sample.json` first.

2. Regenerate the gzip variant of the raw JSON with the `gzip` CLI at max compression:
   ```bash
   cd assets && gzip -9 -k -f compressed_sample.json
   ```

3. Regenerate `.zst` (JSON) and all 3 msgpack variants via Node (no Rust dependency needed for
   this — it's a dev-time step, not a runtime one). `@msgpack/msgpack` isn't a project dependency,
   so install it in a scratch temp directory rather than adding it to any `package.json` in this
   repo:
   ```bash
   mkdir -p /tmp/assetgen && cd /tmp/assetgen && npm init -y >/dev/null 2>&1 && npm install @msgpack/msgpack >/dev/null 2>&1
   ```
   Then write and run a generator script (adjust the repo path):
   ```javascript
   // /tmp/assetgen/gen.mjs
   import { encode } from '@msgpack/msgpack';
   import { readFileSync, writeFileSync } from 'node:fs';
   import zlib from 'node:zlib';

   const jsonPath = process.argv[2];
   const outDir = process.argv[3];
   const jsonText = readFileSync(jsonPath, 'utf8');
   const obj = JSON.parse(jsonText);

   const ZSTD_MAX = { params: { [zlib.constants.ZSTD_c_compressionLevel]: 22 } }; // "ultra" max level
   const GZIP_MAX = { level: zlib.constants.Z_BEST_COMPRESSION };

   const jsonBuf = Buffer.from(jsonText, 'utf8');
   writeFileSync(`${outDir}/compressed_sample.json.gz`, zlib.gzipSync(jsonBuf, GZIP_MAX));
   writeFileSync(`${outDir}/compressed_sample.json.zst`, zlib.zstdCompressSync(jsonBuf, ZSTD_MAX));

   const mp = Buffer.from(encode(obj));
   writeFileSync(`${outDir}/compressed_sample.msgpack`, mp);
   writeFileSync(`${outDir}/compressed_sample.msgpack.gz`, zlib.gzipSync(mp, GZIP_MAX));
   writeFileSync(`${outDir}/compressed_sample.msgpack.zst`, zlib.zstdCompressSync(mp, ZSTD_MAX));
   ```
   ```bash
   node /tmp/assetgen/gen.mjs /path/to/repo/assets/compressed_sample.json /path/to/repo/assets
   ```
   Use `zlib.constants.ZSTD_c_compressionLevel: 22` (not the library default, which is much lower —
   this made a real difference, ~35% smaller, when this was first set up) and
   `Z_BEST_COMPRESSION` (gzip level 9) — both are "maximum compression" settings, matching the
   project's confirmed preference.

4. **Verify round-trip correctness before trusting the new files** — decode all 5 and confirm they
   match the source JSON:
   ```bash
   node -e "
   const { decode } = require('@msgpack/msgpack');
   const zlib = require('zlib');
   const fs = require('fs');
   const dir = 'assets';
   console.log(JSON.parse(zlib.gunzipSync(fs.readFileSync(dir + '/compressed_sample.json.gz')).toString('utf8')));
   console.log(JSON.parse(zlib.zstdDecompressSync(fs.readFileSync(dir + '/compressed_sample.json.zst')).toString('utf8')));
   console.log(decode(fs.readFileSync(dir + '/compressed_sample.msgpack')));
   console.log(decode(zlib.gunzipSync(fs.readFileSync(dir + '/compressed_sample.msgpack.gz'))));
   console.log(decode(zlib.zstdDecompressSync(fs.readFileSync(dir + '/compressed_sample.msgpack.zst'))));
   "
   ```
   (run from a directory with `@msgpack/msgpack` installed, e.g. the same `/tmp/assetgen`, via
   `NODE_PATH` or by running the check from inside that temp directory with an absolute path to
   the repo's `assets/`).

5. `cargo build` — confirms `include_bytes!` picks up the new files (no code changes needed unless
   sizes change enough to matter for tests, which they haven't so far).

6. Sanity-check over the wire before committing: run the server locally (`cargo run`, remember
   `$env:PORT`), curl each `/api/compressed*` endpoint and confirm `Content-Length` matches the new
   file sizes, and check one STOMP topic (SUBSCRIBE + SEND) delivers the same byte count.

7. Clean up the scratch directory (`/tmp/assetgen`) — it must never be committed, and no
   `package.json`/`node_modules` should end up in this repo (the project has zero Node.js runtime
   dependencies; `@msgpack/msgpack` is a dev-time-only tool used to *generate* one of the assets,
   never imported by the Rust server or checked into this repo).
