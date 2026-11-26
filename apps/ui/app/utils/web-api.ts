// Браузерные API, которых нет на «небезопасных» адресах. Сайт, открытый по http не с localhost
// (своя установка по http://192.168.1.10:8400, домен Dokploy без сертификата), — не secure context:
// там нет crypto.randomUUID и navigator.clipboard. Без запасного пути интерфейс падает на старте.
/* eslint-disable no-restricted-properties -- здесь и есть запасной путь */

/** UUID v4: crypto.randomUUID, а где его нет — из crypto.getRandomValues (он есть везде) */
export function uuid(): string {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  const b = crypto.getRandomValues(new Uint8Array(16))
  b[6] = (b[6]! & 0x0F) | 0x40
  b[8] = (b[8]! & 0x3F) | 0x80
  const h = Array.from(b, x => x.toString(16).padStart(2, '0')).join('')
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`
}

/** Текст в буфер обмена; без navigator.clipboard — через скрытое поле и команду copy */
export async function copyText(text: string): Promise<void> {
  if (navigator.clipboard) return navigator.clipboard.writeText(text)
  const field = document.createElement('textarea')
  field.value = text
  field.readOnly = true
  field.style.position = 'fixed'
  field.style.opacity = '0'
  document.body.append(field)
  field.select()
  const copied = document.execCommand('copy')
  field.remove()
  if (!copied) throw new Error('copy is not available')
}
