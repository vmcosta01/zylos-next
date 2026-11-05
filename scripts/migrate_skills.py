#!/usr/bin/env python3
"""
Task 5.3.3: Migração de skills Hermes → zylos-skills registry
Varre SKILL.md files, extrai frontmatter YAML e gera index.json
compatível com SkillsIndex/SkillEntry do crate zylos-skills.
"""

import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

HERMES_SKILLS_DIRS = [
    Path("C:/Users/Paulo/AppData/Local/hermes/skills"),
    Path("C:/Users/Paulo/AppData/Local/hermes/plugins"),
]

OUTPUT_DIR = Path("C:/Users/Paulo/zylos-next/skills")
OUTPUT_FILE = OUTPUT_DIR / "index.json"

FRONTMATTER_RE = re.compile(r"^---\s*\n(.*?)\n---", re.DOTALL)


def parse_yaml_frontmatter(content: str) -> dict:
    """Parse minimal YAML frontmatter (no external dependency)."""
    match = FRONTMATTER_RE.match(content)
    if not match:
        return {}

    yaml_text = match.group(1)
    result = {}
    current_key = None
    current_list = None

    for line in yaml_text.split("\n"):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue

        # List item under current key
        if stripped.startswith("- ") and current_list is not None:
            val = stripped[2:].strip().strip('"').strip("'")
            current_list.append(val)
            continue

        # Key: value
        if ":" in stripped:
            key, _, val = stripped.partition(":")
            key = key.strip()
            val = val.strip()

            if not val:
                # Nested key or list start
                current_key = key
                current_list = []
                result[key] = current_list
            elif val.startswith("[") and val.endswith("]"):
                # Inline list: [a, b, c]
                items = [
                    x.strip().strip('"').strip("'")
                    for x in val[1:-1].split(",")
                    if x.strip()
                ]
                result[key] = items
                current_list = None
            else:
                result[key] = val.strip('"').strip("'")
                current_list = None

    return result


def extract_category_from_path(skill_path: Path) -> str:
    """Extract category from directory structure (e.g., skills/productivity/xlsx)."""
    parts = skill_path.parts
    # Look for known category directories
    for i, part in enumerate(parts):
        if part == "skills" and i + 1 < len(parts):
            candidate = parts[i + 1]
            if candidate not in ("SKILL.md",):
                return candidate
    return "general"


def migrate_skill(skill_md_path: Path) -> dict | None:
    """Parse a single SKILL.md and return a SkillEntry dict."""
    try:
        content = skill_md_path.read_text(encoding="utf-8")
    except Exception as e:
        print(f"  WARN: cannot read {skill_md_path}: {e}", file=sys.stderr)
        return None

    fm = parse_yaml_frontmatter(content)
    if not fm:
        print(f"  SKIP: no frontmatter in {skill_md_path}", file=sys.stderr)
        return None

    name = fm.get("name", "")
    description = fm.get("description", "")
    version = str(fm.get("version", "1.0.0"))

    if not name:
        # Fallback: use directory name
        name = skill_md_path.parent.name

    if not description:
        print(f"  SKIP: no description in {skill_md_path}", file=sys.stderr)
        return None

    # Truncate description to 200 chars per SkillEntry constraint
    if len(description) > 200:
        description = description[:197] + "..."

    # Normalize version to semver
    if not re.match(r"^\d+\.\d+\.\d+", version):
        version = f"{version}.0.0" if "." not in version else f"{version}.0"

    # Extract tags from metadata.hermes.tags or top-level tags
    tags = []
    if isinstance(fm.get("metadata"), list):
        # metadata parsed as list of strings — look for hermes tags
        pass
    if isinstance(fm.get("tags"), list):
        tags = fm["tags"][:10]  # max 10 tags per schema

    # Also check inline metadata
    for key in fm:
        if "tag" in key.lower() and isinstance(fm[key], list):
            tags = fm[key][:10]
            break

    category = extract_category_from_path(skill_md_path)

    # Relative path from skills root
    rel_path = str(skill_md_path.relative_to(OUTPUT_DIR.parent.parent.parent))
    # Use forward slashes and make relative to zylos-next/skills/
    try:
        rel_path = str(skill_md_path.relative_to(OUTPUT_DIR))
    except ValueError:
        # Fallback: use last 3 path components
        rel_path = "/".join(skill_md_path.parts[-3:])

    return {
        "name": name,
        "description": description,
        "version": version,
        "path": rel_path,
        "category": category,
        "tags": tags,
        "depends_on": [],
        "active": True,
    }


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    all_entries = []
    seen_names = set()
    scanned = 0

    for base_dir in HERMES_SKILLS_DIRS:
        if not base_dir.exists():
            print(f"WARN: {base_dir} does not exist", file=sys.stderr)
            continue

        for skill_md in base_dir.rglob("SKILL.md"):
            scanned += 1
            entry = migrate_skill(skill_md)
            if entry is None:
                continue

            # Deduplicate by name
            if entry["name"] in seen_names:
                entry["name"] = f"{entry['name']}-{len(seen_names)}"

            seen_names.add(entry["name"])
            all_entries.append(entry)

    # Sort by name for deterministic output
    all_entries.sort(key=lambda e: e["name"])

    index = {
        "version": "1.0.0",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "total_skills": len(all_entries),
        "skills": all_entries,
    }

    OUTPUT_FILE.write_text(
        json.dumps(index, indent=2, ensure_ascii=False), encoding="utf-8"
    )

    print(f"Scanned: {scanned} SKILL.md files")
    print(f"Migrated: {len(all_entries)} skills")
    print(f"Output:   {OUTPUT_FILE}")
    print(f"Size:     {OUTPUT_FILE.stat().st_size:,} bytes")

    # Print category breakdown
    categories = {}
    for e in all_entries:
        cat = e.get("category", "unknown")
        categories[cat] = categories.get(cat, 0) + 1

    print("\nCategories:")
    for cat, count in sorted(categories.items()):
        print(f"  {cat}: {count}")


if __name__ == "__main__":
    main()