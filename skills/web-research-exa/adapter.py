"""Exa semantic search adapter with proof grounding."""
import os
from typing import Optional


PROVIDER = "exa"
API_BASE = "https://api.exa.ai/v1"
RATE_LIMIT_RPM = 20


def get_api_key() -> str:
    key = os.environ.get("EXA_API_KEY", "")
    if not key:
        raise RuntimeError("EXA_API_KEY not set")
    return key


async def search(query: str, num_results: int = 10, api_key: Optional[str] = None) -> dict:
    """Search via Exa API and return grounded results."""
    import httpx
    key = api_key or get_api_key()
    async with httpx.AsyncClient(timeout=30) as client:
        resp = await client.post(
            f"{API_BASE}/search",
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
            json={"query": query, "numResults": num_results, "useAutoprompt": True},
        )
        resp.raise_for_status()
        data = resp.json()

    return {
        "provider": PROVIDER,
        "query": query,
        "results": [
            {
                "title": r.get("title", ""),
                "url": r.get("url", ""),
                "snippet": r.get("text", "")[:500],
                "proof_level": "observed",
                "source_attribution": r.get("url", ""),
                "published_date": r.get("publishedDate"),
            }
            for r in data.get("results", [])
        ],
        "metadata": {"total": len(data.get("results", []))},
    }
