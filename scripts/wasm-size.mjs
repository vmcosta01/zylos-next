// Quanto cada crate pesa no .wasm: soma o corpo de cada função (seção Code) e agrupa pelo crate dono
// (seção Name, símbolos Rust v0). Uso: node scripts/wasm-size.mjs <arquivo.wasm>. Só lê o arquivo.
import fs from 'fs';

const buf = fs.readFileSync(process.argv[2]);
let pos = 8;
const u32 = () => { let r = 0, s = 0, b; do { b = buf[pos++]; r |= (b & 0x7f) << s; s += 7; } while (b & 0x80); return r >>> 0; };

const secoes = {};
let nomes = new Map();
let importadas = 0;
const corpos = [];
while (pos < buf.length) {
  const id = buf[pos++]; const len = u32(); const fim = pos + len;
  secoes[id] = (secoes[id] || 0) + len;
  if (id === 2) { // imports: conta funções importadas (deslocam o índice)
    const n = u32();
    for (let i = 0; i < n; i++) { let l = u32(); pos += l; l = u32(); pos += l; const k = buf[pos++]; if (k === 0) { u32(); importadas++; } else if (k === 1) { pos++; const f = u32(); u32(); if (f & 1) u32(); } else if (k === 2) { const f = u32(); u32(); if (f & 1) u32(); } else if (k === 3) { pos += 2; } }
  } else if (id === 10) {
    const n = u32();
    for (let i = 0; i < n; i++) { const tam = u32(); corpos.push(tam); pos += tam; }
  } else if (id === 0) {
    const l = u32(); const nome = buf.slice(pos, pos + l).toString(); pos += l;
    if (nome === 'name') {
      while (pos < fim) {
        const sub = buf[pos++]; const sl = u32(); const sfim = pos + sl;
        if (sub === 1) { const n = u32(); for (let i = 0; i < n; i++) { const idx = u32(); const ln = u32(); nomes.set(idx, buf.slice(pos, pos + ln).toString()); pos += ln; } }
        pos = sfim;
      }
    }
  }
  pos = fim;
}
console.log('total', buf.length, 'bytes; seções (id:bytes):', Object.entries(secoes).map(([k, v]) => `${k}:${v}`).join(' '));
console.log('funções com corpo:', corpos.length, '| com nome:', nomes.size);

const grupos = new Map();
const chave = n => {
  if (!n) return '(sem nome)';
  if (n.startsWith('_R')) {
    // Rust v0: o primeiro "C<hash>_<len><crate>" é o crate dono do item.
    const m = n.match(/C[A-Za-z0-9]*_(\d+)([A-Za-z_][A-Za-z0-9_]*)/);
    if (m) return m[2].slice(0, Number(m[1]));
    return '(v0 sem crate)';
  }
  const seg = [...n.matchAll(/(\d+)([A-Za-z_][\w$.]*)/g)].map(x => x[2].slice(0, Number(x[1])));
  return seg[0] || n.slice(0, 24);
};
corpos.forEach((tam, i) => {
  const nome = nomes.get(i + importadas);
  const k = chave(nome);
  const g = grupos.get(k) || { bytes: 0, n: 0 };
  g.bytes += tam; g.n++; grupos.set(k, g);
});
const total = corpos.reduce((a, b) => a + b, 0);
console.log('código total:', total);
for (const [k, g] of [...grupos.entries()].sort((a, b) => b[1].bytes - a[1].bytes).slice(0, 25)) {
  console.log(String(g.bytes).padStart(8), (100 * g.bytes / total).toFixed(1).padStart(5) + '%', String(g.n).padStart(5), k);
}
