"""Firecrawl web scraping adapter with proof grounding."""
import os
from typing import Optional


PROVIDER = "firecrawl"
API_BASE = "https://api.firecrawl.dev/v1"
RATE_LIMIT_RPM = 15


def get_api_key() -> str:
    key = os.environ.get("FIRECRAWL_API_KEY", "")
    if not key:
        raise RuntimeError("FIRECRAWL_API_KEY not set")
    return key


async def scrape(url: str, api_key: Optional[str] = None) -> dict:
    """Scrape a URL via Firecrawl and return grounded content."""
    import httpx
    key = api_key or get_api_key()
    async with httpx.AsyncClient(timeout=60) as client:
        resp = await client.post(
            f"{API_BASE}/scrape",
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
            json={"url": url, "formats": ["markdown"]},
        )
        resp.raise_for_status()
        data = resp.json()

    content = data.get("data", {}).get("markdown", "")
    return {
        "provider": PROVIDER,
        "url": url,
        "content": content,
        "proof_level": "confirmed",
        "source_attribution": url,
        "metadata": data.get("data", {}).get("metadata", {}),
    }


async def search(query: str, num_results: int = 5, api_key: Optional[str] = None) -> dict:
    """Search via Firecrawl and return grounded results."""
    import httpx
    key = api_key or get_api_key()
    async with httpx.AsyncClient(timeout=30) as client:
        resp = await client.post(
            f"{API_BASE}/search",
            headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
            json={"query": query, "limit": num_results},
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
                "snippet": r.get("description", ""),
                "proof_level": "observed",
                "source_attribution": r.get("url", ""),
            }
            for r in data.get("data", [])
        ],
        "metadata": {"total": len(data.get("data", []))},
    }
