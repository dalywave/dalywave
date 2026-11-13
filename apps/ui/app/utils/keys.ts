// Подписи клавиш. В коде сочетания пишутся по-маковски (⌘ K, ⌘ ⇧ M) — на клавиатуре Apple так и
// показываются, на Windows и Linux (сайт и приложение) превращаются в Ctrl K, Ctrl Shift M.
// Обработчики слушают и metaKey, и ctrlKey (useShortcuts) — меняется только подпись

const PC: Record<string, string> = { '⌘': 'Ctrl', '⌃': 'Ctrl', '⌥': 'Alt', '⇧': 'Shift' }

/** Клавиатура Apple: Mac, iPhone, iPad — по строке браузера (у приложения тоже WebView с ней) */
export function isAppleUa(ua: string): boolean {
  return /Macintosh|Mac OS X|iPhone|iPad|iPod/.test(ua)
}

let apple: boolean | null = null
export function isAppleKeyboard(): boolean {
  if (typeof navigator === 'undefined') return true
  apple ??= isAppleUa(navigator.userAgent)
  return apple
}

/** Клавиши сочетания для подписи на этой платформе */
export function platformKeys(keys: readonly string[], appleKeyboard = isAppleKeyboard()): string[] {
  return appleKeyboard ? [...keys] : keys.map(k => PC[k] ?? k)
}
