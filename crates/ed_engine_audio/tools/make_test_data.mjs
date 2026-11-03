// Builds the parity-test fixtures in tests/data from the kit's reference
// renders (the JS is the source of truth):
//
//   node vendor/engine-sound-kit/render.mjs ref/ mustang huracan gt3 f1 civic hayabusa jet
//   node crates/ed_engine_audio/tools/make_test_data.mjs ref/ mustang huracan gt3 f1 civic hayabusa jet
//
// For each car: <id>.controls.json (the controls up to the last window, with
// default-valued keys left out) and <id>.ref.wav (16-bit mono, the JS render's
// (L+R)/2 over the WINDOWS below, back to back).
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const WINDOWS = [[2.0, 2.5], [5.5, 6.0], [10.0, 10.5], [17.15, 17.65]];   // idle, limiter, pull, lift
const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, '..', 'tests', 'data');
const [refDir, ...ids] = process.argv.slice(2);
const END = WINDOWS[WINDOWS.length - 1][1];

for (const id of ids) {
  const c = JSON.parse(readFileSync(join(refDir, `${id}.controls.json`), 'utf8'));
  const frames = c.frames.filter(f => f.t < END + 0.02).map(f => {
    const o = {};
    for (const [k, v] of Object.entries(f)) {
      if (v === 0 || (Array.isArray(v) && v.length === 0)) continue;
      o[k] = v;
    }
    return o;
  });
  writeFileSync(join(out, `${id}.controls.json`), JSON.stringify({ ...c, frames }));

  const wav = readFileSync(join(refDir, `${id}.wav`));
  const sr = wav.readUInt32LE(24);
  const mono = [];
  for (const [a, b] of WINDOWS) {
    for (let i = Math.round(a * sr); i < Math.round(b * sr); i++) {
      const l = wav.readInt16LE(44 + i * 4) / 32767, r = wav.readInt16LE(46 + i * 4) / 32767;
      mono.push(Math.max(-32767, Math.min(32767, Math.round((l + r) / 2 * 32767))));
    }
  }
  const n = mono.length, buf = Buffer.alloc(44 + n * 2);
  buf.write('RIFF', 0); buf.writeUInt32LE(36 + n * 2, 4); buf.write('WAVE', 8);
  buf.write('fmt ', 12); buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(1, 22);
  buf.writeUInt32LE(sr, 24); buf.writeUInt32LE(sr * 2, 28); buf.writeUInt16LE(2, 32); buf.writeUInt16LE(16, 34);
  buf.write('data', 36); buf.writeUInt32LE(n * 2, 40);
  mono.forEach((v, i) => buf.writeInt16LE(v, 44 + i * 2));
  writeFileSync(join(out, `${id}.ref.wav`), buf);
  console.log(id, frames.length, 'frames,', n, 'samples');
}
