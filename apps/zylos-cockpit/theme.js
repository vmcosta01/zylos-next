// Aplica o tema salvo antes da primeira pintura (sem piscar). Sem valor salvo, o CSS segue o sistema.
try {
  var t = localStorage.getItem('zylos-theme');
  if (t === 'light' || t === 'dark') document.documentElement.setAttribute('data-theme', t);
} catch (e) { /* storage bloqueado: segue o sistema */ }
