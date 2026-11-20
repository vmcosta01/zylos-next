// Carrega o módulo wasm "extras" (painéis secundários) só quando uma aba secundária é aberta.
// extras.js é o glue "no-modules" gerado pelo Trunk (data-type="worker"): define a variável global
// `wasm_bindgen`, que depois de inicializada expõe `mount_extra`. O nome não tem hash (ver _headers).
window.__zylosLoadExtras = function () {
  if (window.__zylosExtrasPromise) return window.__zylosExtrasPromise;
  window.__zylosExtrasPromise = new Promise(function (resolve, reject) {
    var s = document.createElement('script');
    s.src = '/extras.js';
    s.onload = function () {
      wasm_bindgen({ module_or_path: '/extras_bg.wasm' }).then(function () { resolve({ mount_extra: wasm_bindgen.mount_extra }); }, reject);
    };
    s.onerror = function () { reject(new Error('extras.js indisponível')); };
    document.head.appendChild(s);
  }).catch(function (e) {
    window.__zylosExtrasPromise = null; // permite tentar de novo
    throw e;
  });
  return window.__zylosExtrasPromise;
};
