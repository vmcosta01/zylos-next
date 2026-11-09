"""Parallel AI search adapter with proof grounding."""
import os
from typing import Optional


PROVIDER = "parallel"
API_BASE = "https://api.parallel.ai/v1"
RATE_LIMIT_RPM = 30


def get_api_key() -> str:
    key = os.environ.get("PARALLEL_API_KEY", "")
    if not key:
        raise RuntimeError("PARALLEL_API_KEY not set")
    return key


async def search(query: str, num_results: int = 10, api_key: Optional[str] = None) -> dict:
    """Search via Parallel API and return grounded results."""
    import httpx
    key = api_key or get_api_key()
    async with httpx.AsyncClient(timeout=30) as client:
        resp = await client.post(
            f"{API_BASE}/search",
            headers={"Authorization": f"Bearer {key}"},
            json={"query": query, "num_results": num_results},
        )
        resp.raise_for_status()
        data = resp.json()

    # Ground through proof levels
    return {
        "provider": PROVIDER,
        "query": query,
        "results": [
            {
                "title": r.get("title", ""),
                "url": r.get("url", ""),
                "snippet": r.get("snippet", ""),
                "proof_level": "observed",
                "source_attribution": r.get("url", ""),
            }
            for r in data.get("results", [])
        ],
        "metadata": {"total": len(data.get("results", [])), "rate_limit_remaining": resp.headers.get("x-ratelimit-remaining")},
    }
