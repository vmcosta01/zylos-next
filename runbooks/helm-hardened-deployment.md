# Runbook: Helm Chart Hardened Deployment — Zylos Gateway

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Deploy em cluster K8s de produção
**Público-alvo:** SRE / Platform Engineer
**Pré-requisitos:** kubectl, helm v3.12+, Prometheus Operator (para ServiceMonitor)

---

## 1. Componentes do Chart

| Template | Função | Condicional |
|----------|--------|-------------|
| deployment.yaml | Pods do gateway | Sempre |
| service.yaml | ClusterIP service | Sempre |
| hpa.yaml | Autoscaling com custom metrics | autoscaling.enabled |
| pdb.yaml | PodDisruptionBudget | podDisruptionBudget.enabled |
| networkpolicy.yaml | Restrição de egress | networkPolicy.enabled |
| servicemonitor.yaml | Prometheus scraping | monitoring.serviceMonitor.enabled |
| grafana-dashboard-cm.yaml | Dashboard JSON auto-import | monitoring.grafanaDashboard.enabled |

## 2. Custom Metrics HPA

| Métrica | Target | Fonte |
|---------|--------|-------|
| zylos_active_agents | 20 avg | Application metrics |
| zylos_queue_depth | 50 avg | Queue system |
| CPU utilization | 80% | Resource |
| Memory utilization | 80% | Resource |

Scaling behavior:
- Scale-up: estabilização 60s, max +2 pods/minuto
- Scale-down: estabilização 300s, max -1 pod/2min

## 3. Network Policy

Egress restrito a:
- DNS (UDP/TCP 53)
- HTTPS/HTTP para CIDRs aprovados (`networkPolicy.allowedEgressCIDRs`)

## 4. Deploy

```bash
helm upgrade --install zylos-gateway ./helm/zylos-gateway   --namespace zylos --create-namespace   -f values-production.yaml
```

## 5. Validação

```bash
kubectl get hpa,pdb,networkpolicy,servicemonitor -n zylos
helm lint ./helm/zylos-gateway
```

## 6. Grafana Dashboard

Painéis: Active Agents, Request Rate, Response Latency (p95), Error Rate, Queue Depth, Memory Usage.

Auto-importado via ConfigMap label `grafana_dashboard: "1"`.

## 7. Referências

- Chart: `helm/zylos-gateway/`
- Dashboard: `helm/zylos-gateway/dashboards/zylos-gateway.json`
- Docs: `docs/IMPLEMENTATION_PLAN.md` §4.4
