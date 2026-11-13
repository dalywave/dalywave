// Классические полосы прокрутки занимают место (Windows и WebView2, Linux, Mac с «Показывать полосы
// прокрутки: всегда»), накладные — нет (macOS по умолчанию, iOS, телефоны). Для классических на корень
// ставится data-scrollbars="classic" — по нему main.css рисует тонкую полосу без дорожки и стрелок;
// накладные остаются системными
export default defineNuxtPlugin(() => {
  const root = document.documentElement
  function measure() {
    const probe = document.createElement('div')
    probe.style.cssText = 'position:absolute;top:-200px;left:0;width:100px;height:100px;overflow:scroll;visibility:hidden'
    document.body.appendChild(probe)
    const classic = probe.offsetWidth - probe.clientWidth > 0
    probe.remove()
    if (classic) root.dataset.scrollbars = 'classic'
    else delete root.dataset.scrollbars
  }
  measure()
  // На Mac настройку меняют на ходу (или подключают мышь при «Автоматически») — окно получает фокус
  window.addEventListener('focus', measure)
})
