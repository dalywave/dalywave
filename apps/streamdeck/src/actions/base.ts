// Общее для всех действий: учёт экземпляров на устройствах, перерисовка по изменениям состояния
// (одинаковое повторно не шлётся — у Stream Deck ограничена частота картинок), долгое нажатие,
// состояние «не подключено» и «нет связи».
import streamDeck, {
  type Action, type DialAction, type DialDownEvent, type DialRotateEvent, type DialUpEvent, type DidReceiveSettingsEvent,
  type KeyAction, type KeyDownEvent, type KeyUpEvent, SingletonAction, type TouchTapEvent,
  type WillAppearEvent, type WillDisappearEvent,
} from '@elgato/streamdeck'
import type { JsonObject } from '@elgato/utils'
import { account } from '../account'
import { t } from '../i18n'
import { player } from '../player'
import { C } from '../render/svg'
import { statusKey, stripGlyph } from '../render/keys'

/** Дольше этого — долгое нажатие */
const LONG_PRESS_MS = 550

export type Problem = 'connect' | 'offline' | 'noDevice' | null

/** Что возвращает обработчик: false — не вышло (кнопка покажет ошибку) */
export type Outcome = void | boolean | Promise<void | boolean>

export interface Instance<S extends JsonObject> {
  action: Action<S>
  settings: S
  /** Последнее отправленное: картинка кнопки и поля полосы */
  image?: string
  title?: string
  layout?: string
  feedback: Record<string, string>
  /** Крутилки: где стоит курсор списка */
  cursor: number
  /** Своё состояние действия (режим крутилки и т. п.) */
  data: Record<string, unknown>
}

export abstract class DeckAction<S extends JsonObject = JsonObject> extends SingletonAction<S> {
  protected instances = new Map<string, Instance<S>>()
  private pressTimers = new Map<string, NodeJS.Timeout>()
  private longFired = new Set<string>()
  /** Перерисовывать раз в секунду, пока играет (прогресс, таймер) */
  protected ticking = false
  /** Действие не требует активного устройства (только связи) */
  protected needsDevice = true

  constructor() {
    super()
    player.on('change', () => this.renderAll())
    player.on('tick', () => {
      if (this.ticking && this.instances.size) this.renderAll()
    })
    account.onChange(() => this.renderAll())
  }

  // ---------- жизненный цикл ----------

  override onWillAppear(ev: WillAppearEvent<S>): void {
    this.instances.set(ev.action.id, { action: ev.action, settings: ev.payload.settings, feedback: {}, cursor: 0, data: {} })
    void this.appeared(this.instances.get(ev.action.id)!)
    this.renderOne(ev.action.id)
  }

  override onWillDisappear(ev: WillDisappearEvent<S>): void {
    this.instances.delete(ev.action.id)
    const timer = this.pressTimers.get(ev.action.id)
    if (timer) clearTimeout(timer)
    this.pressTimers.delete(ev.action.id)
  }

  override onDidReceiveSettings(ev: DidReceiveSettingsEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst) return
    inst.settings = ev.payload.settings
    void this.settingsChanged(inst)
    this.renderOne(ev.action.id)
  }

  /** Новый экземпляр: загрузить свои данные */
  protected appeared(_inst: Instance<S>): void | Promise<void> {}

  /** Настройки поменяли в инспекторе */
  protected settingsChanged(_inst: Instance<S>): void | Promise<void> {}

  renderAll(): void {
    for (const id of this.instances.keys()) this.renderOne(id)
  }

  renderOne(id: string): void {
    const inst = this.instances.get(id)
    if (!inst) return
    const problem = this.problem()
    try {
      if (problem) this.renderProblem(inst, problem)
      else if (inst.action.isKey()) this.renderKey(inst as Instance<S> & { action: KeyAction<S> })
      else if (inst.action.isDial()) this.renderDial(inst as Instance<S> & { action: DialAction<S> })
      else if (inst.action.isNeoInfobar()) this.renderNeo(inst)
    }
    catch (e) {
      streamDeck.logger.error(`render ${this.manifestId}: ${String(e)}`)
    }
  }

  /** Почему действие не может работать */
  problem(): Problem {
    const c = player.connection
    if (c === 'unpaired') return 'connect'
    if (c === 'offline') return 'offline'
    if (c === 'online' && this.needsDevice && !player.target) return 'noDevice'
    return null
  }

  protected renderProblem(inst: Instance<S>, problem: Exclude<Problem, null>): void {
    const label = problem === 'connect' ? t().connect : problem === 'offline' ? t().offline : t().noDevice
    if (inst.action.isKey()) {
      this.setImage(inst, statusKey(problem, label))
    }
    else if (inst.action.isDial()) {
      this.setLayout(inst, 'layouts/list.json')
      const g = problem === 'connect' ? 'plugs-bold' : problem === 'offline' ? 'cloud-slash-bold' : 'devices-bold'
      this.feedback(inst, { cover: stripGlyph(g, 64, C.text3, 0.6), title: label, subtitle: '', counter: '' })
    }
    else if (inst.action.isNeoInfobar()) {
      this.feedback(inst, { cover: stripGlyph('cloud-slash-bold', 42, C.text3, 0.6), title: label, subtitle: '' })
    }
  }

  protected renderKey(_inst: Instance<S> & { action: KeyAction<S> }): void {}
  protected renderDial(_inst: Instance<S> & { action: DialAction<S> }): void {}
  protected renderNeo(_inst: Instance<S>): void {}

  // ---------- вывод без повторов ----------

  protected setImage(inst: Instance<S>, image: string): void {
    if (inst.image === image || !inst.action.isKey()) return
    inst.image = image
    void inst.action.setImage(image)
  }

  /** Подпись Stream Deck поверх картинки; пустая — человек может задать свою в настройках кнопки */
  protected setTitle(inst: Instance<S>, title: string): void {
    if (inst.title === title) return
    inst.title = title
    if (inst.action.isKey() || inst.action.isDial()) void inst.action.setTitle(title)
  }

  protected setLayout(inst: Instance<S>, layout: string): void {
    if (inst.layout === layout) return
    inst.layout = layout
    inst.feedback = {}
    if (inst.action.isDial() || inst.action.isNeoInfobar()) void inst.action.setFeedbackLayout(layout)
  }

  /** Поля полосы: строка — значение, объект — свойства элемента. Шлётся только изменённое */
  protected feedback(inst: Instance<S>, fields: Record<string, string | number | JsonObject>): void {
    const diff: Record<string, string | number | JsonObject> = {}
    for (const [k, v] of Object.entries(fields)) {
      const key = typeof v === 'object' ? JSON.stringify(v) : String(v)
      if (inst.feedback[k] === key) continue
      inst.feedback[k] = key
      diff[k] = v
    }
    if (!Object.keys(diff).length) return
    if (inst.action.isDial() || inst.action.isNeoInfobar()) void inst.action.setFeedback(diff as never)
  }

  // ---------- нажатия ----------

  /** Короткое нажатие кнопки */
  protected press(_inst: Instance<S>): Outcome {}
  /** Долгое нажатие; нет — короткое срабатывает сразу при нажатии */
  protected longPress?(inst: Instance<S>): Outcome

  override onKeyDown(ev: KeyDownEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst) return
    inst.settings = ev.payload.settings
    // Не подключено: нажатие начинает вход — страница подтверждения откроется в браузере
    if (this.problem() === 'connect') {
      if (account.pairing.phase !== 'waiting') void account.start()
      return
    }
    if (!this.longPress) {
      void this.run(inst, () => this.press(inst))
      return
    }
    this.longFired.delete(inst.action.id)
    this.pressTimers.set(inst.action.id, setTimeout(() => {
      this.pressTimers.delete(inst.action.id)
      this.longFired.add(inst.action.id)
      void this.run(inst, () => this.longPress?.(inst))
    }, LONG_PRESS_MS))
  }

  override onKeyUp(ev: KeyUpEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst || !this.longPress) return
    const timer = this.pressTimers.get(inst.action.id)
    if (timer) {
      clearTimeout(timer)
      this.pressTimers.delete(inst.action.id)
      void this.run(inst, () => this.press(inst))
    }
    this.longFired.delete(inst.action.id)
  }

  /** Выполнить и показать ошибку на кнопке, если не вышло */
  protected async run(inst: Instance<S>, fn: () => unknown): Promise<void> {
    try {
      const ok = await fn()
      if (ok === false && (inst.action.isKey() || inst.action.isDial())) void inst.action.showAlert()
    }
    catch (e) {
      streamDeck.logger.warn(`${this.manifestId}: ${String(e)}`)
      if (inst.action.isKey() || inst.action.isDial()) void inst.action.showAlert()
    }
  }

  // ---------- крутилки ----------

  protected rotate(_inst: Instance<S>, _ticks: number, _pressed: boolean): Outcome {}
  protected dialPress(_inst: Instance<S>): Outcome {}
  protected touch(inst: Instance<S>, hold: boolean): Outcome {
    if (!hold) return this.dialPress(inst)
  }

  override onDialRotate(ev: DialRotateEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst || this.problem()) return
    inst.settings = ev.payload.settings
    if (ev.payload.pressed) inst.data.rotatedWhilePressed = true
    void this.run(inst, () => this.rotate(inst, ev.payload.ticks, ev.payload.pressed))
  }

  override onDialDown(ev: DialDownEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst) return
    inst.data.rotatedWhilePressed = false
  }

  override onDialUp(ev: DialUpEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst) return
    if (this.problem() === 'connect') {
      if (account.pairing.phase !== 'waiting') void account.start()
      return
    }
    if (this.problem()) return
    // Покрутили с нажатием — это не нажатие
    if (inst.data.rotatedWhilePressed) return
    void this.run(inst, () => this.dialPress(inst))
  }

  override onTouchTap(ev: TouchTapEvent<S>): void {
    const inst = this.instances.get(ev.action.id)
    if (!inst || this.problem()) return
    void this.run(inst, () => this.touch(inst, ev.payload.hold))
  }
}
