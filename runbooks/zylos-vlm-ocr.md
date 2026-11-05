# Runbook — zylos-vlm-ocr (VLM OCR Sidecar)

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Crate:** `crates/zylos-vlm-ocr`
**Dependências:** `serde`, `serde_json`, `tokio`, `base64`
**Fase:** 4 (SANDBOX_ISOLATION) → 7 (END_TO_END_INTEGRATION)

---

## 1. Visão Geral

O `zylos-vlm-ocr` é um sidecar de OCR que extrai texto estruturado de imagens usando Vision Language Models (VLM). Projetado para processar screenshots, documentos escaneados e capturas de UI geradas por agentes.

**Módulos:**
- `lib.rs` — API pública: `OcrEngine`, `OcrResult`, `OcrConfig`
- `svg_parser.rs` — Parser SVG especializado para diagramas e fluxogramas (17 testes)
- `error.rs` — Tipos de erro unificados (`OcrError`)

**Integração:** Usado pelo `zylos-sandbox` como pós-processador de screenshots e pelo `zylos-live-comm` para extração de texto de capturas de UI em tempo real.

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que sidecar e não library embutida?
- **Isolamento de dependências pesadas:** VLM backends (ONNX, TensorRT) são opcionais e volumosos
- **Escalabilidade independente:** Pode rodar como processo separado com GPU dedicada
- **Fail-safe:** Falha no OCR não derruba o agente principal
- **Swap de backend:** Trocar provedor VLM sem recompilar crates consumidores

### 2.2 SVG Parser especializado
Screenshots de UI e diagramas frequentemente contêm SVGs embutidos. O parser nativo:
- Extrai `<text>` elements com posição e estilo
- Reconhece nós de fluxograma (retângulos + setas + labels)
- Preserva hierarquia visual (parent-child relationships)
- 17 testes cobrindo edge cases: namespaces, CDATA, transforms, gradients

### 2.3 Backends suportados
| Backend | Status | Use case |
|---------|--------|----------|
| Local ONNX | ✅ Default | Offline, baixa latência |
| OpenAI Vision API | ✅ | Alta precisão, documentos complexos |
| Google Gemini Vision | ✅ | Multi-language, handwriting |
| Custom HTTP endpoint | ✅ | Self-hosted models |

Backend selecionado via `OcrConfig::backend` ou env `ZYLOS_OCR_BACKEND`.

### 2.4 Trade-offs
| Aspecto | Nossa Abordagem | Alternativa Rejeitada |
|---------|----------------|----------------------|
| Integração | Sidecar (processo/library) | Library embutida (deps pesadas) |
| SVG | Parser customizado | resvg (sem extração semântica) |
| Cache | Hash da imagem → resultado | Sem cache (reprocessamento caro) |
| Batch | Async pipeline com bounded queue | Sync blocking (throughput baixo) |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-vlm-ocr
# 17 testes: svg_text_extraction, svg_namespace_handling, svg_cdata_parsing,
# svg_transform_coordinates, svg_gradient_skip, svg_nested_groups, etc.
```

### 3.2 API pública
```rust
use zylos_vlm_ocr::{OcrEngine, OcrConfig, OcrBackend};

// Configurar engine
let config = OcrConfig {
    backend: OcrBackend::LocalOnnx,
    cache_enabled: true,
    max_concurrent: 4,
};
let engine = OcrEngine::new(config)?;

// Processar imagem (bytes PNG/JPEG/SVG)
let result = engine.process(image_bytes).await?;

// Acessar texto extraído
println!("Texto: {}", result.text);
println!("Confiança: {:.2}", result.confidence);
println!("Regiões: {:?}", result.regions);
```

### 3.3 Variáveis de ambiente
| Variável | Default | Descrição |
|----------|---------|-----------|
| `ZYLOS_OCR_BACKEND` | `local_onnx` | Backend VLM (`local_onnx`, `openai`, `gemini`, `custom`) |
| `ZYLOS_OCR_CACHE_DIR` | `./ocr_cache` | Diretório de cache de resultados |
| `ZYLOS_OCR_MAX_CONCURRENT` | `4` | Limite de processamento paralelo |
| `OPENAI_API_KEY` | — | Chave API OpenAI (backend openai) |
| `GOOGLE_API_KEY` | — | Chave API Google (backend gemini) |

### 3.4 Checklist operacional
- [ ] Backend configurado conforme ambiente (local para dev, cloud para prod)
- [ ] Cache directory com permissão de escrita
- [ ] API keys configuradas para backends cloud
- [ ] Modelo ONNX baixado (se backend local)
- [ ] Testes passando (17/17)
- [ ] Métricas de latência dentro do target (<2s para screenshot 1080p)

### 3.5 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| `Model not found` | Arquivo ONNX ausente | Baixar modelo ou mudar backend |
| `API key missing` | Variável de ambiente não setada | Exportar chave API |
| `SVG parse error` | SVG malformado ou unsupported feature | Validar SVG, reportar bug se válido |
| `Cache write failed` | Permissão ou disco cheio | Verificar `ZYLOS_OCR_CACHE_DIR` |
| `Timeout processing` | Imagem muito grande ou backend lento | Reduzir resolução ou aumentar timeout |
| `Low confidence` | Imagem borrada ou texto pequeno | Pré-processar (sharpen, upscale) |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-vlm-ocr
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-vlm-ocr
```
**Esperado:** `test result: ok. 17 passed`.

### 4.3 Testar SVG parser isoladamente
```bash
cargo test -p zylos-vlm-ocr svg -- --nocapture
```
**Esperado:** Todos os testes SVG passam com output detalhado.

### 4.4 Verificar cache funcionando
```bash
ls -la ./ocr_cache/  # ou diretório configurado em ZYLOS_OCR_CACHE_DIR
```
**Esperado:** Arquivos `.json` com resultados cacheados após primeiro processamento.

### 4.5 Quando escalar
- Erro de compilação → verificar dependências (`serde`, `base64`, `tokio`)
- Teste SVG falha → ler mensagem, comparar SVG input com expected output
- Backend cloud retorna 401 → verificar API key e billing
- Modelo ONNX não carrega → verificar integridade do arquivo (`sha256sum`)
- Latência alta → profiler com `cargo flamegraph` ou verificar tamanho da imagem
- Cache inconsistente → limpar `rm -rf ./ocr_cache/*` e reprocessar

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-vlm-ocr/src/`
- **Runbook Fase 4:** `runbooks/fase4-sandbox-isolation.md`
- **Runbook VLM OCR:** `runbooks/vlm-ocr-sidecar.md` (versão detalhada)
- **Security model:** `specs/SECURITY-MODEL.md` §Data Processing
- **HANDOFF_4:** Documentação Fase 4