"""Fallback chain: parallel → exa → firecrawl."""
import asyncio
from typing import Optional


async def search_with_fallback(query: str, num_results: int = 10) -> dict:
    """Try providers in order until one succeeds."""
    providers = [
        ("parallel", "skills.web-research-parallel.adapter"),
        ("exa", "skills.web-research-exa.adapter"),
        ("firecrawl", "skills.web-research-firecrawl.adapter"),
    ]
    
    errors = []
    for name, module_path in providers:
        try:
            import importlib
            mod = importlib.import_module(module_path)
            result = await mod.search(query, num_results)
            result["fallback_chain"] = {"used": name, "skipped": [p[0] for p in providers if p[0] != name]}
            return result
        except Exception as e:
            errors.append({"provider": name, "error": str(e)})
    
    return {
        "error": "All providers failed",
        "fallback_errors": errors,
        "query": query,
    }
