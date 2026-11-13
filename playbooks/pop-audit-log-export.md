# POP-004: Exportar Audit Log

**Version:** 1.0.0
**Playbook:** [pb-approval-gate-setup.md](./pb-approval-gate-setup.md)
**Last Updated:** 2026-11-11

## Purpose

Exportar entradas do audit log em formato CSV para auditoria externa, compliance ou análise forense. O export inclui todos os campos exceto arguments (que são armazenados apenas como hash SHA-256).

## Trigger

- Solicitação de auditoria externa (compliance, legal)
- Investigação de incidente de segurança
- Relatório mensal de atividades dos agentes
- Backup antes de rotação de logs (retenção 90 dias)

## Steps

### 1. Definir filtros de export

```bash
# Exemplo: exportar ações do bot-1 nos últimos 7 dias
curl -s "http://localhost:3000/api/v1/audit/export?bot_id=bot-1&since=2026-11-04T00:00:00Z&format=csv" \
 -o audit_export_bot1_7d.csv
```

Parâmetros disponíveis:
- `bot_id`: filtrar por bot específico
- `action_type`: filtrar por tipo de ação (send_email, deploy, payment, etc.)
- `approval_status`: filtrar por status (approved, refused, expired, auto_approved)
- `since`: data inicial (ISO 8601)
- `until`: data final (ISO 8601)
- `format`: csv (padrão) ou json

### 2. Verificar integridade do export

```bash
# Contar linhas (deve bater com total da API)
wc -l audit_export_bot1_7d.csv

# Verificar header
head -1 audit_export_bot1_7d.csv
# Esperado: id,bot_id,action_type,target,approval_status,approval_decision,arguments_hash,result_status,result_summary,created_at,decided_at,executed_at

# Verificar que arguments_hash tem 64 chars (SHA-256 hex)
awk -F',' 'NR>1 {print length($7)}' audit_export_bot1_7d.csv | sort -u
# Esperado: apenas 64
```

### 3. Validar contra stats da API

```bash
# Comparar contagem do export com stats
curl -s "http://localhost:3000/api/v1/audit/stats?period=7" | jq '.total_actions'
# Deve bater com número de linhas do CSV (menos header)
```

### 4. Armazenar export seguro

- Salvar em diretório com acesso restrito (apenas security/compliance team)
- Nomear com timestamp: `audit_export_YYYYMMDD_HHMMSS.csv`
- Gerar checksum SHA-256 do arquivo para integridade futura
- Se contiver dados sensíveis (target emails), criptografar em repouso

## Verification

- CSV tem header correto com 12 colunas
- Número de linhas bate com `/audit/stats` para mesmo período
- `arguments_hash` tem exatamente 64 caracteres hex em todas as linhas
- Nenhum campo plaintext de argumentos ou senhas no export
- Arquivo salvo em local seguro com permissões restritas

## Rollback

Export é operação read-only — não há rollback necessário. Se export incorreto foi gerado:
1. Deletar arquivo exportado
2. Corrigir filtros e re-exportar
3. Verificar novo export antes de distribuir

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Export vazio quando deveria ter dados | Platform team | Verificar se migrations foram aplicadas |
| arguments_hash não tem 64 chars | Dev team | Bug no hash function, severity high |
| Dados sensíveis em plaintext no export | Security team + Incident Response | Imediatamente, severity critical |
| Performance lenta (>30s para 10k rows) | Dev team | Otimizar query/indexes |