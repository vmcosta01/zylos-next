// Zylos Cockpit — Service Worker (PWA Offline-First)
// Task 6.2.1: PWA offline-first with queue replay on reconnect
// Cache strategy: App Shell (cache-first) + API (network-first) + Events (stream-only)

const CACHE_NAME = 'zylos-cockpit-v1';
const APP_SHELL = [
  '/',
  '/index.html',
  '/pkg/zylos-cockpit.js',
  '/pkg/zylos-cockpit_bg.wasm',
  '/manifest.json',
  '/icons/icon-192.png',
  '/icons/icon-512.png'
];

// Install: pre-cache app shell
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(APP_SHELL);
    })
  );
  self.skipWaiting();
});

// Activate: clean old caches, claim clients immediately
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((names) => {
      return Promise.all(
        names
          .filter((name) => name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      );
    }).then(() => self.clients.claim())
  );
});

// Fetch: network-first for API, cache-first for static assets, stream passthrough
self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // SSE/WebSocket streams: never cache, always network
  if (url.pathname === '/events' || url.pathname === '/api/v1/events') {
    return; // Let browser handle natively
  }

  // API endpoints: network-first with cache fallback
  if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/health') || url.pathname.startsWith('/ready')) {
    event.respondWith(
      fetch(event.request)
        .then((response) => {
          // Update cache with fresh response
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
          return response;
        })
        .catch(() => {
          // Fallback to cached response for GET requests
          if (event.request.method === 'GET') {
            return caches.match(event.request);
          }
          // For non-GET, queue for replay on reconnect
          return enqueueRequest(event.request);
        })
    );
    return;
  }

  // Static assets: cache-first with network fallback
  event.respondWith(
    caches.match(event.request).then((cached) => {
      if (cached) return cached;
      return fetch(event.request).then((response) => {
        // Cache successful responses for static assets
        if (response.status === 200) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
        }
        return response;
      });
    })
  );
});

// Offline queue for failed mutations (POST/PUT/DELETE)
const QUEUE_DB = 'zylos-offline-queue';
const QUEUE_STORE = 'pending-requests';

async function enqueueRequest(request) {
  try {
    const db = await openQueueDB();
    const tx = db.transaction(QUEUE_STORE, 'readwrite');
    const entry = {
      id: crypto.randomUUID(),
      url: request.url,
      method: request.method,
      headers: Object.fromEntries(request.headers.entries()),
      body: request.body ? await request.arrayBuffer() : null,
      timestamp: Date.now()
    };
    tx.objectStore(QUEUE_STORE).add(entry);
    await new Promise((resolve, reject) => {
      tx.oncomplete = resolve;
      tx.onerror = reject;
    });
    return new Response(JSON.stringify({ queued: true, id: entry.id }), {
      status: 202,
      headers: { 'Content-Type': 'application/json' }
    });
  } catch (e) {
    return new Response(JSON.stringify({ error: 'Queue unavailable' }), {
      status: 503,
      headers: { 'Content-Type': 'application/json' }
    });
  }
}

function openQueueDB() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(QUEUE_DB, 1);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(QUEUE_STORE)) {
        db.createObjectStore(QUEUE_STORE, { keyPath: 'id' });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

// Replay queued requests when back online
self.addEventListener('online', async () => {
  try {
    const db = await openQueueDB();
    const tx = db.transaction(QUEUE_STORE, 'readwrite');
    const store = tx.objectStore(QUEUE_STORE);
    const all = await new Promise((resolve) => {
      const req = store.getAll();
      req.onsuccess = () => resolve(req.result);
    });

    for (const entry of all) {
      try {
        await fetch(entry.url, {
          method: entry.method,
          headers: entry.headers,
          body: entry.body
        });
        // Remove from queue on success
        store.delete(entry.id);
      } catch {
        // Keep in queue for next retry
      }
    }
  } catch {
    // Queue DB unavailable; skip replay
  }
});

// Background sync for queued mutations
self.addEventListener('sync', (event) => {
  if (event.tag === 'zylos-replay-queue') {
    event.waitUntil(replayAllQueued());
  }
});

async function replayAllQueued() {
  const db = await openQueueDB();
  const tx = db.transaction(QUEUE_STORE, 'readwrite');
  const store = tx.objectStore(QUEUE_STORE);
  const all = await new Promise((resolve) => {
    const req = store.getAll();
    req.onsuccess = () => resolve(req.result);
  });

  const succeeded = [];
  for (const entry of all) {
    try {
      const response = await fetch(entry.url, {
        method: entry.method,
        headers: entry.headers,
        body: entry.body
      });
      if (response.ok || response.status < 500) {
        succeeded.push(entry.id);
      }
    } catch {
      // Retry later
    }
  }

  for (const id of succeeded) {
    store.delete(id);
  }
}