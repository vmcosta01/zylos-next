# Runbook: Zylos Cockpit — Frontend WASM, PWA e Temas

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** UI não carrega, tema quebrado, SSE desconecta, ou problema de performance no frontend
**Público-alvo:** Frontend Dev / Support / QA
**Pré-requisitos:** Browser moderno (Chrome/Firefox/Edge), DevTools, acesso ao cockpit URL

---

## 1. Arquitetura

O Cockpit é o painel web do Zylos:
- **WASM core**: Lógica de renderização compilada de Rust para WebAssembly
- **PWA**: Service Worker para offline-first e caching
- **SSE**: Server-Sent Events para updates em tempo real
- **Temas**: CSS variables com suporte a dark/light/custom
- **State**: Reactive store sincronizado com gateway via SSE

## 2. Verificação de Saúde

```bash
# Health check do frontend
curl -s https://cockpit.example.com/health

# Versão do build
curl -s https://cockpit.example.com/version.json | jq .

# WASM bundle size
curl -sI https://cockpit.example.com/pkg/zylos_cockpit_bg.wasm | grep Content-Length
```

No browser DevTools:
- Console: zero erros críticos
- Network: SSE connection aberta (EventStream)
- Application > Service Workers: ativo e sem erros
- Performance: FCP < 2s, TTI < 3s

## 3. Troubleshooting

### 3.1 Tela branca / WASM não carrega
```
DevTools > Console:
- "Failed to fetch WASM" → Verificar CDN/network
- "RuntimeError: unreachable" → Bug no Rust code, abrir issue
- "WebAssembly.instantiateStreaming failed" → MIME type incorreto

Fix:
1. Hard refresh (Ctrl+Shift+R)
2. Clear cache + service worker unregister
3. Verificar Content-Type: application/wasm no server
```

### 3.2 SSE desconecta frequentemente
```
DevTools > Network > EventStream:
- Verificar retry interval
- Checar se há proxy/load balancer bufferizando SSE

Configurar no gateway:
  sse.keepalive_interval: 15s
  sse.retry_timeout: 30s

Workaround client-side:
  localStorage.setItem('zylos_sse_fallback', 'polling')
```

### 3.3 Tema não aplica / cores erradas
```css
/* Verificar CSS variables no DevTools > Elements > Computed */
--zylos-bg-primary
--zylos-text-primary
--zylos-accent

/* Reset tema para default */
localStorage.removeItem('zylos_theme')
location.reload()

/* Aplicar tema custom */
document.documentElement.setAttribute('data-theme', 'custom')
```

### 3.4 PWA não funciona offline
```
DevTools > Application > Service Workers:
- Verificar status "activated"
- Checar cache storage

Re-registrar SW:
navigator.serviceWorker.register('/sw.js').then(r => console.log(r))

Clear caches:
caches.keys().then(ks => ks.forEach(k => caches.delete(k)))
```

### 3.5 Performance degradada
```
DevTools > Performance:
- Record 10s de interação
- Verificar long tasks (>50ms)
- Checar memory leaks (Heap snapshot diff)

WASM profiling:
- Habilitar chrome://flags/#enable-webassembly-profiling
- Usar perf-record no Rust side
```

## 4. Build e Deploy

```bash
# Build WASM
cd apps/zylos-cockpit
wasm-pack build --target web --release

# Preview local
npm run dev

# Production build
npm run build
# Output: dist/ (static files)

# Deploy
rsync -avz dist/ deploy@cdn:/var/www/cockpit/
```

## 5. Validação Final

- [ ] Cockpit carrega em < 3s (Lighthouse Performance > 90)
- [ ] SSE mantém conexão estável por > 1h
- [ ] Tema dark/light troca instantaneamente
- [ ] PWA funciona offline (cache hit)
- [ ] Zero console errors em fluxo normal
- [ ] WASM bundle < 500KB gzipped

## 6. Referências

- Playbook: `playbooks/deploy-release.md` (para frontend releases)
- SOAR: `docs/soar/runbooks/zylos-cockpit.runbook.yaml`
- Runbook relacionado: `runbooks/fase6-cockpit-wasm.md` (build detalhado)
