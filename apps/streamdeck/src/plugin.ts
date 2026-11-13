// Плагин DalyWave для Stream Deck: пульт музыки на кнопках, крутилках и полосе Stream Deck +.
// Управляет тем устройством, где сейчас играет звук (сайт, Mac, Windows, iPhone), через сервер.
import streamDeck from '@elgato/streamdeck'
import { account, type GlobalSettings } from './account'
import { AddToAction, CollectionAction, DiscoverAction, QueueAction, SearchAction, TransferAction, WaveAction } from './actions/lists'
import {
  DislikeAction, LikeAction, MultimediaAction, NotNowAction, NowPlayingAction, PlaybackAction, RepeatAction,
  ShuffleAction, SleepAction, TransitionAction, VolumeAction, sleepTimer,
} from './actions/transport'
import { api } from './api'
import { setLanguage } from './i18n'
import { setupInspector } from './inspector'
import { player } from './player'

declare const PLUGIN_VERSION: string

streamDeck.logger.setLevel('info')
api.version = typeof PLUGIN_VERSION === 'string' ? PLUGIN_VERSION : '0.0.0'

for (const action of [
  new PlaybackAction(), new ShuffleAction(), new RepeatAction(), new VolumeAction(), new LikeAction(),
  new AddToAction(), new CollectionAction(), new SearchAction(), new QueueAction(), new SleepAction(),
  new NowPlayingAction(), new MultimediaAction(), new WaveAction(), new DiscoverAction(), new TransferAction(),
  new TransitionAction(), new DislikeAction(), new NotNowAction(),
]) {
  streamDeck.actions.registerAction(action)
}

setupInspector()

// Отозвали в «Доступе» или вышли на сайте: кнопки просят подключиться заново
api.onUnauthorized(() => void account.revoked())
player.link.on('unauthorized', () => void account.revoked())

streamDeck.settings.onDidReceiveGlobalSettings<GlobalSettings>(ev => account.received(ev.settings))
streamDeck.system.onSystemDidWakeUp(() => player.link.wake())

await streamDeck.connect()
setLanguage(streamDeck.info.application.language)
await account.load()
sleepTimer.restore()
