# Zylos-Next Training Materials

## Session Overview (4 sessions × 2h)

### Session 1: Architecture & Core Concepts
**Duration:** 2h | **Audience:** All stakeholders

#### Agenda
1. **Welcome & Goals** (10min)
2. **Architecture Overview** (30min)
   - C4 diagrams walkthrough
   - Crate dependency graph
   - Data flow: inbound item → triage → approval → execution → outcome
3. **Core Concepts Deep Dive** (40min)
   - 5-level Ground enum (Assumed → Proven)
   - Verification gates and proof chains
   - Adversarial panel judging
   - Shadow-git audit trails
4. **Live Demo: Gateway Health Checks** (20min)
   - `/gateway/health`, `/ready`, `/live` endpoints
   - Config hot-reload via SIGHUP
5. **Q&A** (20min)

#### Exercises
- [ ] Start gateway locally and hit all 3 health endpoints
- [ ] Modify `config.toml` and trigger hot-reload
- [ ] Read `docs/ARCHITECTURE.md` and identify your team's crate

---

### Session 2: HITL, Triage & Sentinel Operations
**Duration:** 2h | **Audience:** Devs + Ops

#### Agenda
1. **Two-Stage Triage Engine** (30min)
   - Stage 1 rules (safe negatives only)
   - Stage 2 model fallback
   - `decided_by` audit field
   - Framing patterns for scheduled routines
2. **Equivalence-Based Approval** (30min)
   - SHA-256 stable hashing
   - 4-tier decision ordering (Safety > Team > Personal > Auto > Default)
   - Safety immutability
   - Continuation capture via tokio task-local
3. **Sentinel Health Scanner** (30min)
   - Pure-function detectors
   - Normalized signatures for dedup
   - PII stripping (emails, UUIDs, IPv4, hex tokens)
   - Rate-limited incident reporting (5/hr)
4. **Live Demo: Triage + Approval Flow** (20min)
5. **Q&A** (10min)

#### Exercises
- [ ] Add a custom triage rule (remember: NO NeedsYou from rules!)
- [ ] Submit an approval and verify equivalence hash reuse on retry
- [ ] Feed observations to Sentinel and verify PII is stripped in output
- [ ] Trigger rate limiter and observe suppression

---

### Session 3: Deployment & Monitoring
**Duration:** 2h | **Audience:** Ops + SRE

#### Agenda
1. **Deployment Options** (30min)
   - Bare metal, Docker, Kubernetes/Helm
   - Production checklist walkthrough
   - Migration script (`scripts/migrate-production.sh`)
2. **Monitoring Stack** (30min)
   - Prometheus scrape config
   - Grafana dashboards
   - Alertmanager routing (Telegram/PagerDuty)
3. **Backup & Recovery** (20min)
   - Daily SQLite backups
   - Skills directory archival
   - Rollback procedure
4. **Performance Targets** (20min)
   - Startup <3s, shutdown <5s, health <100ms
   - RAM <30MB/agent idle
   - E2E suite <30min
5. **Live Demo: Production Migration Dry-Run** (20min)

#### Exercises
- [ ] Run `migrate-production.sh --dry-run` and review output
- [ ] Set up Prometheus + Grafana locally
- [ ] Configure alertmanager with test webhook
- [ ] Execute backup script and verify artifacts

---

### Session 4: Incident Response & On-Call
**Duration:** 2h | **Audience:** On-call engineers

#### Agenda
1. **SOAR Workflows** (30min)
   - Health anomaly → triage → approval → remediation
   - Task failure recovery with equivalence approval
   - Triage model drift detection
2. **Incident Response Playbook** (30min)
   - Severity classification
   - Escalation paths (ungated tool)
   - Communication templates
3. **Debugging Common Issues** (30min)
   - Safety tier override failures
   - Lower-tier denial blocking higher-tier
   - PII not stripped in output
   - Backoff/test failures
4. **Simulated Incident Exercise** (30min)

#### Exercises
- [ ] Resolve simulated safety-blocked approval
- [ ] Debug PII stripping failure
- [ ] Execute full incident response playbook
- [ ] Write post-incident report

---

## Prerequisites
- Rust 1.80+ installed
- Docker Desktop running
- Access to staging environment
- Read `README.md` and `docs/ARCHITECTURE.md` before Session 1

## Assessment
Each session ends with a practical exercise. Completion requires:
- All exercises checked off
- Ability to explain key concepts without reference material
- Successful resolution of at least 1 simulated incident

## Materials Distribution
- Slides: `docs/training/session-{1,2,3,4}-slides.pdf`
- Exercises: This document
- Reference: `runbooks/`, `playbooks/`, `docs/FAQ.md`