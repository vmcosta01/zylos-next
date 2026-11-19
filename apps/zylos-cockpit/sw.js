// Zylos Cockpit — Service Worker (Task 6.2.4)
// DESATIVADO TEMPORARIAMENTE durante desenvolvimento v2.0 para evitar cache stale.
// Reativar após deploy de produção.

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});
self.addEventListener('fetch', (event) => {
  // Pass-through: sempre busca da rede, sem cache
  event.respondWith(fetch(event.request).catch(() => new Response('Offline', { status: 503 })));
});