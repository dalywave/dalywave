// Миксы сервера по-английски в английском интерфейсе: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { englishMixes, translateMixes } from '../app/utils/mixTitles'

describe('englishMixes', () => {
  test('переводит миксы по generator и не трогает свои плейлисты', () => {
    const data = {
      items: [
        { kind: 'mix', generator: 'daily_mix', title: 'Микс дня 2', description: 'Burning House, Bluebridge и другие' },
        { kind: 'mix', generator: 'discover_weekly', title: 'Открытия недели', description: 'и другие новые для тебя' },
        { kind: 'mix', generator: 'on_repeat', title: 'На повторе', description: 'Что вы слушаете чаще всего прямо сейчас' },
        { kind: 'user', generator: null, title: 'Микс дня 3', description: '' },
      ],
    }
    expect(englishMixes(data)).toBe(true)
    expect(data.items.map(p => p.title)).toEqual(['Daily Mix 2', 'Discover Weekly', 'On Repeat', 'Микс дня 3'])
    expect(data.items.map(p => p.description)).toEqual(['Burning House, Bluebridge and more', 'New to you', 'What you play most right now', ''])
    expect(englishMixes(data)).toBe(false)
  })

  test('переводит засеянные сочетания волны, а свои — нет', () => {
    const data = { items: [
      { name: 'Работа', icon: 'laptop', settings: {} },
      { name: 'Дорога', icon: 'car', settings: {} },
      { name: 'Моя дорога', icon: 'car', settings: {} },
    ] }
    englishMixes(data)
    expect(data.items.map(p => p.name)).toEqual(['Work', 'Road', 'Моя дорога'])
  })
})

describe('translateMixes uk', () => {
  test('переводит миксы и сочетания на украинский, повторный проход ничего не меняет', () => {
    const data = {
      items: [
        { kind: 'mix', generator: 'daily_mix', title: 'Микс дня 2', description: 'Burning House, Bluebridge и другие' },
        { kind: 'mix', generator: 'discover_weekly', title: 'Открытия недели', description: 'и другие новые для тебя' },
        { kind: 'mix', generator: 'repeat_rewind', title: 'Старые хиты на повторе', description: 'Когда-то крутились без остановки' },
        { name: 'Ночь', icon: 'moon-stars', settings: {} },
        { name: 'Дорога', icon: 'car', settings: {} },
      ],
    }
    expect(translateMixes(data, 'uk')).toBe(true)
    expect(data.items.map(p => ('title' in p ? p.title : p.name))).toEqual(['Мікс дня 2', 'Відкриття тижня', 'Старі хіти на повторі', 'Ніч', 'Дорога'])
    expect(data.items.map(p => ('description' in p ? p.description : ''))).toEqual(['Burning House, Bluebridge та інші', 'Нове для тебе', 'Колись крутилися без упину', '', ''])
    expect(translateMixes(data, 'uk')).toBe(false)
  })
})
