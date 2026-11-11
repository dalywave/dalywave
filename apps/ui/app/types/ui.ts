// Общие типы компонентов интерфейса

export interface MenuAction {
  kind?: 'item'
  label: string
  icon?: string
  kbd?: string[]
  danger?: boolean
  disabled?: boolean
  /** Отмеченный пункт (радио/галочка) */
  checked?: boolean
  hint?: string
  onSelect?: () => void
  children?: MenuEntry[]
}

export interface MenuSeparator {
  kind: 'separator'
}

export interface MenuHeading {
  kind: 'heading'
  label: string
  hint?: string
}

export type MenuEntry = MenuAction | MenuSeparator | MenuHeading

export interface ToastOptions {
  text: string
  icon?: string
  tone?: 'default' | 'error'
  /** Кнопка «Отменить»: вызывается, если человек передумал */
  undo?: () => void
  action?: { label: string, onClick: () => void }
  duration?: number
}
