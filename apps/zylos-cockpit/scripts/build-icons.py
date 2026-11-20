"""Gera fonts/phosphor.css e fonts/phosphor-subset.woff2 só com os ícones usados no cockpit.

Fonte: Phosphor Icons (MIT) 2.1.1, estilo "regular", em scripts/phosphor-src/ (fonte inteira:
~147 KB; o subconjunto servido em fonts/ fica com poucos KB).

Uso (na pasta apps/zylos-cockpit):
    uv run --with fonttools --with brotli python scripts/build-icons.py

Ao usar um ícone novo no Rust (`ph ph-NOME`), acrescente NOME em ICONS e rode de novo.
"""
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FONTS = HERE.parent / "fonts"
SRC = HERE / "phosphor-src"

ICONS = """
robot check-square brain target user-circle graph share-network shield-check lightning tree-structure
funnel lock-key hand-palm books eye scales megaphone device-mobile chats-circle chat-teardrop-text smiley
toolbox pencil-simple git-branch folder-simple chart-line-up users-three scroll microphone user-focus
palette buildings globe-hemisphere-west plugs-connected tray pulse
sun moon circle-half caret-down caret-right caret-left x check warning-circle shield-warning
magnifying-glass plus arrows-out-simple hand-grabbing camera list folder
check-circle x-circle info warning clock-counter-clockwise file-text game-controller hand-waving clipboard-text
arrows-clockwise broadcast floppy-disk gear question sword traffic-signal upload-simple wrench
""".split()

css = (SRC / "phosphor-full.css").read_text(encoding="utf-8")
rules = dict(re.findall(r'\.ph\.ph-([a-z0-9-]+):before\s*\{\s*content:\s*"\\([0-9a-f]+)";\s*\}', css))

missing = [n for n in ICONS if n not in rules]
if missing:
    sys.exit(f"ícones que não existem no Phosphor: {missing}")

codepoints = [rules[n] for n in ICONS]
out_css = [
    "/* Gerado por scripts/build-icons.py (Phosphor Icons, MIT). Não editar à mão. */",
    '@font-face {',
    '  font-family: "Phosphor";',
    '  src: url("/fonts/phosphor-subset.woff2") format("woff2");',
    "  font-weight: normal;",
    "  font-style: normal;",
    "  font-display: swap;",
    "}",
    ".ph {",
    '  font-family: "Phosphor" !important;',
    "  font-style: normal;",
    "  font-weight: normal;",
    "  line-height: 1;",
    "  text-transform: none;",
    "  letter-spacing: 0;",
    "  display: inline-block;",
    "  -webkit-font-smoothing: antialiased;",
    "}",
]
for n in ICONS:
    out_css.append(f'.ph-{n}::before {{ content: "\\{rules[n]}"; }}')
(FONTS / "phosphor.css").write_text("\n".join(out_css) + "\n", encoding="utf-8")

unicodes = ",".join(f"U+{c.upper()}" for c in codepoints)
subprocess.run(
    [
        sys.executable, "-m", "fontTools.subset", str(SRC / "phosphor.woff2"),
        f"--unicodes={unicodes}", "--flavor=woff2", "--no-layout-closure",
        f"--output-file={FONTS / 'phosphor-subset.woff2'}",
    ],
    check=True,
)
size = (FONTS / "phosphor-subset.woff2").stat().st_size
print(f"{len(ICONS)} ícones, phosphor-subset.woff2 = {size} bytes")
