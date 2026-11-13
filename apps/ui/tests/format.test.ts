// Числа для интерфейса: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { formatCompactCount, NBSP } from '../app/utils/format'

describe('formatCompactCount', () => {
  test('до десяти тысяч — целиком, с формой слова', () => {
    expect(formatCompactCount(1, 'listener')).toBe('1 слушатель')
    expect(formatCompactCount(9_999, 'listener')).toBe(`9${NBSP}999 слушателей`)
  })
  test('больше — коротко; число не разрывается, слово может уйти на вторую строку', () => {
    expect(formatCompactCount(2_523_247, 'listener')).toBe(`2,5${NBSP}млн слушателей`)
    expect(formatCompactCount(18_166, 'listener')).toBe(`18,2${NBSP}тыс. слушателей`)
    expect(formatCompactCount(2_523_247, 'listener', 'en')).toBe('2.5M listeners')
  })
})
