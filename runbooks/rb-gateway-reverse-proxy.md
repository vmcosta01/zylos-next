# RB-Gateway-Reverse-Proxy — Reverse Proxy com TLS e WebSocket para o Gateway

**Versão:** 1.0.0
**Data:** 2026-11-17
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.3
**Artefatos:** [`deploy/reverse-proxy/Caddyfile`](../deploy/reverse-proxy/Caddyfile) (recomendado), [`deploy/reverse-proxy/nginx.conf`](../deploy/reverse-proxy/nginx.conf) (alternativa)
**Relacionados:** [`pop-health-check-validation.md`](./pop-health-check-validation.md), [`rb-production-incident-response.md`](./rb-production-incident-response.md)
**Público:** SRE, DevOps, on-call
**Tempo Estimado:** 20–40 min (primeira instalação)

---

## 1. Visão Geral

O `zylos-gateway` (axum) escuta em `127.0.0.1:3000` (default de `apps/zylos-gateway/src/config.rs`) e **não termina TLS**. Em produção, um reverse proxy na mesma VPS faz:

| Responsabilidade | Caddy | nginx |
|---|---|---|
| TLS termination (TLS 1.2/1.3) | automático (ACME) | certbot + `ssl_*` |
| Redirect HTTP→HTTPS | automático | bloco `listen 80` |
| HSTS + headers de segurança | `header {}` | `add_header ... always` |
| WebSocket `/ws/*` (ex.: `/ws/computer-view/{session_id}`) | automático, `stream_close_delay 5m` | `Upgrade`/`Connection` via `map`, timeouts 1h |
| SSE `/api/v1/events` | `flush_interval -1`, `read_timeout 0` | `proxy_buffering off`, `X-Accel-Buffering no` |
| Health `/api/v1/health`, `/api/v1/cockpit/health` | timeouts de 2s | timeouts de 2s, sem access log |
| Tudo fora de `/api/` e `/ws/` | 404 | 404 |

O cockpit estático **não** passa por este proxy (fica no Cloudflare Pages — ver `pb-cloudflare-pages-setup.md`).

**Escopo de verificação desta entrega:** os arquivos foram validados sintaticamente em containers (ver §5). **Não existe servidor de produção provisionado** (T11.4.2 pendente), então TLS real, ACME e upgrade de WebSocket ponta-a-ponta ainda não foram exercitados.

---

## 2. Nível Senior — Decisões

- **Caddy como padrão:** TLS automático elimina o cron de renovação do certbot e reduz a superfície de erro humano. nginx fica documentado para ambientes onde ele já é padrão.
- **Gateway preso em loopback:** `listen_addr = "127.0.0.1:3000"`. Nunca expor a porta 3000 publicamente; o firewall só libera 80/443 (e 443/udp para HTTP/3 no Caddy).
- **Timeouts de stream longos e explícitos:** WS e SSE ficam abertos por horas; sem `read_timeout` adequado o proxy derruba conexões a cada 60s e o cockpit entra em loop de reconexão.
- **Health check com timeout curto:** alvo de <100ms (critério 11.4); 2s de teto evita que um gateway travado segure conexões no proxy.

## 3. Nível Pleno — Instalação

### 3.1 Caddy (recomendado)

```bash
# Debian/Ubuntu
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy

sudo cp deploy/reverse-proxy/Caddyfile /etc/caddy/Caddyfile
# Variáveis (systemd drop-in)
sudo systemctl edit caddy   # adicionar:
#   [Service]
#   Environment=ZYLOS_GATEWAY_DOMAIN=gateway.seu-dominio
#   Environment=ZYLOS_ACME_EMAIL=ops@seu-dominio
sudo mkdir -p /var/log/caddy && sudo chown caddy:caddy /var/log/caddy
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

Pré-requisito: registro DNS `A/AAAA` do domínio apontando para a VPS **antes** do primeiro start (o desafio ACME HTTP-01/TLS-ALPN-01 precisa alcançar a porta 80/443).

### 3.2 nginx (alternativa)

```bash
sudo apt install -y nginx certbot
sudo sed 's/gateway.zylos.example/gateway.seu-dominio/g' deploy/reverse-proxy/nginx.conf \
  | sudo tee /etc/nginx/sites-available/zylos-gateway.conf
sudo ln -s /etc/nginx/sites-available/zylos-gateway.conf /etc/nginx/sites-enabled/
sudo mkdir -p /var/www/certbot
# 1ª emissão (comente temporariamente o bloco 443 se o certificado ainda não existir)
sudo certbot certonly --webroot -w /var/www/certbot -d gateway.seu-dominio
sudo nginx -t && sudo systemctl reload nginx
# Renovação: o pacote certbot instala um timer systemd; validar com
sudo certbot renew --dry-run
```

### 3.3 Firewall

```bash
sudo ufw allow 80/tcp && sudo ufw allow 443/tcp && sudo ufw allow 443/udp
sudo ufw deny 3000/tcp
```

## 4. Nível Junior — Verificação pós-instalação

```bash
D=gateway.seu-dominio
# TLS + redirect
curl -sI http://$D/api/v1/health | head -1          # esperado: 301/308
curl -sf https://$D/api/v1/health                     # esperado: JSON 200
curl -s -o /dev/null -w '%{time_total}\n' https://$D/api/v1/health   # alvo < 0.1s
# Headers
curl -sI https://$D/api/v1/health | grep -i strict-transport-security
# SSE (deve imprimir eventos sem esperar o fim)
curl -N -H 'Accept: text/event-stream' https://$D/api/v1/events
# WebSocket (101 Switching Protocols)
curl -si --http1.1 -H 'Connection: Upgrade' -H 'Upgrade: websocket' \
  -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' \
  https://$D/ws/computer-view/test | head -1
# Grade TLS externa (opcional): https://www.ssllabs.com/ssltest/
```

## 5. Validação executada nesta entrega (2026-11-17)

| Verificação | Comando | Resultado real |
|---|---|---|
| Caddyfile | `docker run --rm -v .../deploy/reverse-proxy:/etc/caddy:ro caddy:2 caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile` | `Valid configuration` |
| nginx.conf | `nginx:alpine` com certificado autoassinado temporário em `/etc/letsencrypt/live/gateway.zylos.example/` + `nginx -t` | `syntax is ok` / `test is successful` (aviso esperado: `ssl_stapling ignored` por ser autoassinado) |

Não verificado: emissão ACME real, upgrade WS ponta-a-ponta, latência <100ms — dependem do provisionamento (T11.4.2).

## 6. Troubleshooting

| Sintoma | Causa provável | Ação |
|---|---|---|
| Cockpit reconecta SSE a cada ~60s | timeout padrão do proxy | conferir `read_timeout 0` (Caddy) / `proxy_read_timeout 1h` (nginx) no bloco `/api/v1/events` |
| Eventos SSE chegam em rajadas | buffering ativo | `flush_interval -1` / `proxy_buffering off` |
| WS retorna 400/426 | headers de upgrade removidos | nginx: conferir `map $http_upgrade` e `proxy_set_header Upgrade` no `location /ws/` |
| `502 Bad Gateway` | gateway parado ou bind em outra porta | `systemctl status zylos-gateway`; `ss -ltnp | grep 3000`; ver `pop-gateway-restart.md` |
| ACME falha (`challenge failed`) | DNS não propagou ou porta 80 bloqueada | `dig +short $D`; `ufw status` |
| CORS bloqueado no cockpit | origem do Pages não permitida no gateway | ajustar allowlist de CORS do gateway (não do proxy) |

## 7. Rollback

```bash
# Caddy
sudo cp /etc/caddy/Caddyfile.bak /etc/caddy/Caddyfile && sudo systemctl reload caddy
# nginx
sudo rm /etc/nginx/sites-enabled/zylos-gateway.conf && sudo nginx -t && sudo systemctl reload nginx
```

Sempre fazer `cp Caddyfile Caddyfile.bak` (ou equivalente) antes de alterar a configuração em produção.
