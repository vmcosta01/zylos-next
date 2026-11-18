[2m│[0m [1mapps\zylos-cockpit\styles\design-tokens.css[0m
[2m│[0m [38;2;139;134;130m@@ -23,15 +23,62 @@[0m
[2m│[0m [38;2;184;134;11m   --color-status-info: #3b82f6;[0m
[2m│[0m [38;2;184;134;11m   --color-status-idle: #6b7280;[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  /* ── Agent Division Colors ── */[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-engineering: #6366f1;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-research: #8b5cf6;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-operations: #ec4899;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-creative: #f97316;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-security: #ef4444;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-management: #14b8a6;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-support: #3b82f6;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-  --division-custom: #a855f7;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  /* ── Agent Role Colors (21 roles) ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-engineering: #3b82f6;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-security: #ef4444;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-devops: #f59e0b;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-data: #8b5cf6;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-design: #ec4899;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-product: #10b981;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-qa: #06b6d4;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-research: #6366f1;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-support: #14b8a6;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-finance: #84cc16;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-legal: #78716c;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-hr: #f97316;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-marketing: #d946ef;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-operations: #0ea5e9;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-architecture: #a855f7;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-testing: #22d3ee;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-documentation: #fbbf24;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-integration: #34d399;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-monitoring: #fb923c;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-compliance: #a3a3a3;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --role-custom: #6b7280;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  /* ── Topology Node Colors ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --node-agent: #3b82f6;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --node-orchestrator: #f59e0b;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --node-gateway: #10b981;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --node-online: #22c55e;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --node-degraded: #eab308;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --node-offline: #ef4444;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  /* ── Chart Palette (10 series) ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-1: #4f8cff;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-2: #22c55e;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-3: #f59e0b;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-4: #ef4444;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-5: #a855f7;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-6: #06b6d4;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-7: #ec4899;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-8: #84cc16;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-9: #f97316;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --chart-10: #64748b;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  /* ── CRDT Peer Palette (8 peers) ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-1: #e6194b;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-2: #3cb44b;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-3: #4363d8;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-4: #f58231;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-5: #911eb4;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-6: #008080;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-7: #9a6324;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --peer-8: #d6336c;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  /* ── Accent Semantic Aliases ── */[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --accent-blue: #3e63dd;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+  --accent-red: #e5484d;[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m   /* ── Typography ── */[0m
[2m│[0m [38;2;184;134;11m   --font-family-sans: 'Inter', ui-sans-serif, system-ui, -apple-system, 'Segoe UI', sans-serif;[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\agent_avatars.rs[0m
[2m│[0m [38;2;139;134;130m@@ -60,31 +60,37 @@[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m impl AgentDivision {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    /// Retorna a cor hex associada à divisão[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    pub fn color(&self) -> &'static str {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Retorna o nome da CSS custom property associada à divisão.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// O valor real da cor vive em `styles/design-tokens.css` (ADR-008).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn color_var(&self) -> &'static str {[0m
[2m│[0m [38;2;184;134;11m         match self {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Engineering => "#3B82F6",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Security => "#EF4444",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::DevOps => "#F59E0B",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Data => "#8B5CF6",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Design => "#EC4899",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Product => "#10B981",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::QA => "#06B6D4",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Research => "#6366F1",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Support => "#14B8A6",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Finance => "#84CC16",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Legal => "#78716C",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::HR => "#F97316",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Marketing => "#D946EF",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Operations => "#0EA5E9",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Architecture => "#A855F7",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Testing => "#22D3EE",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Documentation => "#FBBF24",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Integration => "#34D399",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Monitoring => "#FB923C",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Compliance => "#A3A3A3",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            Self::Custom(_) => "#6B7280",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Engineering => "--role-engineering",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Security => "--role-security",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::DevOps => "--role-devops",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Data => "--role-data",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Design => "--role-design",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Product => "--role-product",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::QA => "--role-qa",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Research => "--role-research",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Support => "--role-support",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Finance => "--role-finance",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Legal => "--role-legal",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::HR => "--role-hr",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Marketing => "--role-marketing",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Operations => "--role-operations",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Architecture => "--role-architecture",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Testing => "--role-testing",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Documentation => "--role-documentation",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Integration => "--role-integration",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Monitoring => "--role-monitoring",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Compliance => "--role-compliance",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            Self::Custom(_) => "--role-custom",[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Retorna a referência CSS `var(--role-*)` para uso em inline styles.[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    pub fn color(&self) -> String {[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        format!("var({})", self.color_var())[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m     /// Retorna as iniciais da divisão para exibição no avatar[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\agent_avatars.rs[0m
[2m│[0m [38;2;139;134;130m@@ -340,14 +340,14 @@[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m             // Legenda de divisões[0m
[2m│[0m [38;2;184;134;11m             <div class="agent-division-legend">[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #3B82F6"></span>"Engenharia"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #EF4444"></span>"Segurança"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #F59E0B"></span>"DevOps"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #8B5CF6"></span>"Dados"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #EC4899"></span>"Design"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #10B981"></span>"Produto"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #06B6D4"></span>"QA"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                <span class="legend-item"><span class="legend-dot" style="background: #6366F1"></span>"Pesquisa"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-engineering)"></span>"Engenharia"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-security)"></span>"Segurança"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-devops)"></span>"DevOps"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-data)"></span>"Dados"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-design)"></span>"Design"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-product)"></span>"Produto"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-qa)"></span>"QA"</span>[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                <span class="legend-item"><span class="legend-dot" style="background: var(--role-research)"></span>"Pesquisa"</span>[0m
[2m│[0m [38;2;184;134;11m             </div>[0m
[2m│[0m [38;2;184;134;11m         </div>[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\agent_avatars.rs[0m
[2m│[0m [38;2;139;134;130m@@ -382,8 +382,8 @@[0m
[2m│[0m [38;2;184;134;11m             AgentDivision::Compliance,[0m
[2m│[0m [38;2;184;134;11m         ];[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let colors: Vec<&str> = divisions.iter().map(|d| d.color()).collect();[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-        let unique: std::collections::HashSet<&str> = colors.iter().copied().collect();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let colors: Vec<String> = divisions.iter().map(|d| d.color()).collect();[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+        let unique: std::collections::HashSet<&String> = colors.iter().collect();[0m
[2m│[0m [38;2;184;134;11m         assert_eq!([0m
[2m│[0m [38;2;184;134;11m             colors.len(),[0m
[2m│[0m [38;2;184;134;11m             unique.len(),[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\analytics_charts.rs[0m
[2m│[0m [38;2;139;134;130m@@ -15,10 +15,11 @@[0m
[2m│[0m [38;2;184;134;11m use leptos::prelude::*;[0m
[2m│[0m [38;2;184;134;11m use serde::Deserialize;[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-/// Paleta categórica (acessível em tema claro e escuro).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Paleta categórica via CSS custom properties (ADR-008).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Valores definidos em `styles/design-tokens.css` como --chart-1..--chart-10.[0m
[2m│[0m [38;2;184;134;11m pub const PALETTE: [&str; 10] = [[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    "#4f8cff", "#22c55e", "#f59e0b", "#ef4444", "#a855f7", "#06b6d4", "#ec4899", "#84cc16",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    "#f97316", "#64748b",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    "var(--chart-1)", "var(--chart-2)", "var(--chart-3)", "var(--chart-4)", "var(--chart-5)",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    "var(--chart-6)", "var(--chart-7)", "var(--chart-8)", "var(--chart-9)", "var(--chart-10)",[0m
[2m│[0m [38;2;184;134;11m ];[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m /// Rótulos dos dias (0 = segunda, como em `zylos_analytics::HeatmapCell`).[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\swarm_topology_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -28,12 +28,12 @@[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    /// Cor de preenchimento SVG por tipo[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Cor de preenchimento SVG por tipo (CSS var reference, ADR-008)[0m
[2m│[0m [38;2;184;134;11m     pub fn fill_color(&self) -> &'static str {[0m
[2m│[0m [38;2;184;134;11m         match self {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            NodeType::Agent => "#3b82f6",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            NodeType::Orchestrator => "#f59e0b",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            NodeType::Gateway => "#10b981",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            NodeType::Agent => "var(--node-agent)",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            NodeType::Orchestrator => "var(--node-orchestrator)",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            NodeType::Gateway => "var(--node-gateway)",[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\swarm_topology_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -65,12 +65,12 @@[0m
[2m│[0m [38;2;184;134;11m }[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m impl NodeStatus {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    /// Cor do anel de status ao redor do nó[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    /// Cor do anel de status ao redor do nó (CSS var reference, ADR-008)[0m
[2m│[0m [38;2;184;134;11m     pub fn stroke_color(&self) -> &'static str {[0m
[2m│[0m [38;2;184;134;11m         match self {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            NodeStatus::Online => "#22c55e",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            NodeStatus::Degraded => "#eab308",[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-            NodeStatus::Offline => "#ef4444",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            NodeStatus::Online => "var(--node-online)",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            NodeStatus::Degraded => "var(--node-degraded)",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+            NodeStatus::Offline => "var(--node-offline)",[0m
[2m│[0m [38;2;184;134;11m         }[0m
[2m│[0m [38;2;184;134;11m     }[0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\swarm_topology_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -504,11 +504,11 @@[0m
[2m│[0m [38;2;184;134;11m                 .topology-node.dragging { cursor: grabbing; }[0m
[2m│[0m [38;2;184;134;11m                 .topology-tooltip {[0m
[2m│[0m [38;2;184;134;11m                     position: fixed;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    background: #1e293b;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    border: 1px solid #475569;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    background: var(--color-bg-secondary);[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    border: 1px solid var(--color-border-strong);[0m
[2m│[0m [38;2;184;134;11m                     border-radius: 8px;[0m
[2m│[0m [38;2;184;134;11m                     padding: 10px 14px;[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    color: #e2e8f0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    color: var(--color-text-primary);[0m
[2m│[0m [38;2;184;134;11m                     font-size: 12px;[0m
[2m│[0m [38;2;184;134;11m                     line-height: 1.5;[0m
[2m│[0m [38;2;184;134;11m                     pointer-events: none;[0m
[2m│[0m [38;2;139;134;130m@@ -517,16 +517,16 @@[0m
[2m│[0m [38;2;184;134;11m                     max-width: 240px;[0m
[2m│[0m [38;2;184;134;11m                 }[0m
[2m│[0m [38;2;184;134;11m                 .topology-tooltip .tt-label { font-weight: 600; font-size: 13px; margin-bottom: 4px; }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                .topology-tooltip .tt-type { color: #94a3b8; font-size: 11px; }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                .topology-tooltip .tt-type { color: var(--color-text-secondary); font-size: 11px; }[0m
[2m│[0m [38;2;184;134;11m                 .topology-tooltip .tt-status { margin-top: 4px; font-size: 11px; }[0m
[2m│[0m [38;2;184;134;11m                 .topology-controls {[0m
[2m│[0m [38;2;184;134;11m                     display: flex; gap: 6px; margin-bottom: 8px; justify-content: flex-end;[0m
[2m│[0m [38;2;184;134;11m                 }[0m
[2m│[0m [38;2;184;134;11m                 .topology-controls button {[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                    background: #334155; border: 1px solid #475569; color: #e2e8f0;[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                    background: var(--color-bg-tertiary); border: 1px solid var(--color-border-strong); color: var(--color-text-primary);[0m
[2m│[0m [38;2;184;134;11m                     border-radius: 4px; padding: 4px 10px; font-size: 12px; cursor: pointer;[0m
[2m│[0m [38;2;184;134;11m                 }[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                .topology-controls button:hover { background: #475569; }[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                .topology-controls button:hover { background: var(--color-border-strong); }[0m
[2m│[0m [38;2;184;134;11m                 "#}[0m
[2m│[0m [38;2;184;134;11m             </style>[0m
[2m│[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\swarm_topology_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -582,7 +582,7 @@[0m
[2m│[0m [38;2;184;134;11m                                     y1=y1.to_string()[0m
[2m│[0m [38;2;184;134;11m                                     x2=x2.to_string()[0m
[2m│[0m [38;2;184;134;11m                                     y2=y2.to_string()[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                    stroke="#94a3b8"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                    stroke="var(--color-text-secondary)"[0m
[2m│[0m [38;2;184;134;11m                                     stroke-width=stroke_width.to_string()[0m
[2m│[0m [38;2;184;134;11m                                     stroke-dasharray=dash[0m
[2m│[0m [38;2;184;134;11m                                     opacity="0.6"[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\swarm_topology_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -622,7 +622,7 @@[0m
[2m│[0m [38;2;184;134;11m                                         x=node.x.to_string()[0m
[2m│[0m [38;2;184;134;11m                                         y=label_y.to_string()[0m
[2m│[0m [38;2;184;134;11m                                         text-anchor="middle"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                                        fill="#e2e8f0"[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                                        fill="var(--color-text-primary)"[0m
[2m│[0m [38;2;184;134;11m                                         font-size="11"[0m
[2m│[0m [38;2;184;134;11m                                         font-family="system-ui, sans-serif"[0m
[2m│[0m [38;2;184;134;11m                                         style="pointer-events: none; user-select: none;"[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\analytics_panel.rs[0m
[2m│[0m [38;2;139;134;130m@@ -330,7 +330,7 @@[0m
[2m│[0m [38;2;184;134;11m                 <div class="an-grid-charts">[0m
[2m│[0m [38;2;184;134;11m                     <LineChart title="Execuções por período" labels=labels.clone() values=counts />[0m
[2m│[0m [38;2;184;134;11m                     <LineChart title="Taxa de sucesso (%)" labels=labels values=rates unit="%"[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-                        max=100.0 color="#22c55e" />[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+                        max=100.0 color="var(--color-status-success)" />[0m
[2m│[0m [38;2;184;134;11m                     <BarChart title="Sucesso por skill (%)" items=skill_bars unit="%" max=100.0[0m
[2m│[0m [38;2;184;134;11m                         warn_below=70.0 />[0m
[2m│[0m [38;2;184;134;11m                     <PieChart title="Volume por agente" items=agent_pie />[0m
[2m│[0m [1mapps\zylos-cockpit\src\crdt_client.rs[0m
[2m│[0m [38;2;139;134;130m@@ -30,9 +30,11 @@[0m
[2m│[0m [38;2;184;134;11m /// Tag `Message::Custom` de ping/pong (igual a `crdt_sync::PING_TAG`).[0m
[2m│[0m [38;2;184;134;11m pub const PING_TAG: u8 = 100;[0m
[2m│[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-/// Paleta de cores de presença (contraste AA sobre fundo escuro e claro).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Paleta de cores de presença via CSS custom properties (ADR-008).[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+/// Valores definidos em `styles/design-tokens.css` como --peer-1..--peer-8.[0m
[2m│[0m [38;2;184;134;11m pub const PRESENCE_COLORS: [&str; 8] = [[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-    "#e6194b", "#3cb44b", "#4363d8", "#f58231", "#911eb4", "#008080", "#9a6324", "#d6336c",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    "var(--peer-1)", "var(--peer-2)", "var(--peer-3)", "var(--peer-4)",[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+    "var(--peer-5)", "var(--peer-6)", "var(--peer-7)", "var(--peer-8)",[0m
[2m│[0m [38;2;184;134;11m ];[0m
[2m│[0m
[2m│[0m [38;2;184;134;11m /// Cor determinística para um client id.[0m
[2m│[0m [1mapps\zylos-cockpit\src\components\agent_avatars.rs[0m
[2m│[0m [38;2;139;134;130m@@ -4,28 +4,28 @@[0m
[2m│[0m [38;2;184;134;11m //! Fornece avatares coloridos baseados na divisão/função do agente,[0m
[2m│[0m [38;2;184;134;11m //! com mais de 20 cores distintas para diferenciação visual clara.[0m
[2m│[0m [38;2;184;134;11m //![0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! ## Divisões e Cores[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Engineering: #3B82F6 (azul)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Security: #EF4444 (vermelho)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - DevOps: #F59E0B (âmbar)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Data: #8B5CF6 (violeta)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Design: #EC4899 (rosa)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Product: #10B981 (esmeralda)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - QA: #06B6D4 (ciano)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Research: #6366F1 (índigo)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Support: #14B8A6 (teal)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Finance: #84CC16 (lima)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Legal: #78716C (pedra)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - HR: #F97316 (laranja)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Marketing: #D946EF (fúcsia)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Operations: #0EA5E9 (sky)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Architecture: #A855F7 (roxo)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Testing: #22D3EE (ciano claro)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Documentation: #FBBF24 (âmbar claro)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Integration: #34D399 (esmeralda claro)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Monitoring: #FB923C (laranja claro)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Compliance: #A3A3A3 (cinza)[0m
[2m│[0m [38;2;255;255;255;48;2;119;20;20m-//! - Default: #6B7280 (cinza médio)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! ## Divisões e Cores (CSS custom properties em `styles/design-tokens.css`)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Engineering: var(--role-engineering)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Security: var(--role-security)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - DevOps: var(--role-devops)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Data: var(--role-data)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Design: var(--role-design)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Product: var(--role-product)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - QA: var(--role-qa)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Research: var(--role-research)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Support: var(--role-support)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Finance: var(--role-finance)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Legal: var(--role-legal)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - HR: var(--role-hr)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Marketing: var(--role-marketing)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Operations: var(--role-operations)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Architecture: var(--role-architecture)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Testing: var(--role-testing)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Documentation: var(--role-documentation)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Integration: var(--role-integration)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Monitoring: var(--role-monitoring)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Compliance: var(--role-compliance)[0m
[2m│[0m [38;2;255;255;255;48;2;19;87;20m+//! - Default: var(--role-custom)[0m
[2m│[0m [38;2;184;134;11m //![0m
[2m│[0m [38;2;184;134;11m //! ## Related Documents[0m
[2m│[0m [38;2;184;134;11m //! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.6[0m
Fase 1 of the Zylos Cockpit v2.0 redesign is complete and pushed to master (commit `47e8608`).

**Verified gates:**
- Zero hardcoded colors in `src/` (excluding `skin_themes.rs` theme definitions and `sentinel_panel.rs` CSS fallbacks)
- `cargo check --target wasm32-unknown-unknown`: passed
- `trunk build --dist` (wasm-release): passed
- `cargo test --lib`: 115 tests passed, 0 failed
- `cargo clippy --all-targets -- -D warnings`: clean
- Pushed: `ccbaa8a..47e8608 master -> master`

**Changes delivered:**
- Extended `styles/design-tokens.css` with 47 new semantic tokens: 21 role colors, 6 topology node colors, 10 chart palette, 8 CRDT peer colors, 2 accent aliases
- Migrated 6 Rust source files from hardcoded hex to `var(--token-name)` references
- Updated doc comments to reference CSS custom properties

Ready to proceed with Fase 2 (Layout Shell + Translucency) when you give the go-ahead.

session_id: local-82c9b8aa7da3386725cbc0aa7303a5b276f33a92daf674334a0e629f4542fd2b
