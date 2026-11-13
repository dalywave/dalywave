// Описание AAC для MP4. WebCodecs обещает в `decoderConfig.description` AudioSpecificConfig (2 байта), а Safari
// отдаёт весь ES_Descriptor из esds. mp4-muxer заворачивает описание в esds ещё раз — и плееры видят «AAC
// типа 0, 0 каналов», звука нет. Здесь из ES_Descriptor достаётся сам AudioSpecificConfig. Без DOM — bun test

const FREQS = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350]

/** AudioSpecificConfig AAC-LC по частоте и числу каналов — когда описания нет совсем */
export function aacLcConfig(sampleRate: number, channels: number): Uint8Array {
  const idx = FREQS.indexOf(sampleRate)
  if (idx < 0) throw new Error(`AAC: частота ${sampleRate} без индекса`)
  // 5 бит тип (2 — LC), 4 бита частота, 4 бита каналы, 3 бита нули
  return new Uint8Array([(2 << 3) | (idx >> 1), ((idx & 1) << 7) | (channels << 3)])
}

/** Дескриптор MPEG-4: тег и длина (до 4 байт по 7 бит) */
function header(b: Uint8Array, at: number): { tag: number, size: number, body: number } | null {
  if (at >= b.length) return null
  const tag = b[at]!
  let size = 0
  let i = at + 1
  for (let n = 0; n < 4 && i < b.length; n++) {
    const v = b[i++]!
    size = (size << 7) | (v & 0x7f)
    if (!(v & 0x80)) break
  }
  return { tag, size, body: i }
}

/** Найти DecSpecificInfo (тег 5) внутри ES_Descriptor (3) → DecoderConfigDescriptor (4) */
function findDecSpecific(b: Uint8Array, from: number, to: number): Uint8Array | null {
  let at = from
  while (at < to) {
    const h = header(b, at)
    if (!h || h.body + h.size > b.length) return null
    const end = h.body + h.size
    if (h.tag === 0x05) return b.slice(h.body, end)
    if (h.tag === 0x03) {
      // ES_ID (2), флаги (1) и то, что флаги добавляют
      let p = h.body + 2
      const flags = b[p++] ?? 0
      if (flags & 0x80) p += 2
      if (flags & 0x40) p += 1 + (b[p] ?? 0)
      if (flags & 0x20) p += 2
      const found = findDecSpecific(b, p, end)
      if (found) return found
    }
    else if (h.tag === 0x04) {
      // objectTypeIndication, streamType, bufferSize (3), maxBitrate (4), avgBitrate (4)
      const found = findDecSpecific(b, h.body + 13, end)
      if (found) return found
    }
    at = end
  }
  return null
}

function bytes(src: AllowSharedBufferSource): Uint8Array {
  if (src instanceof Uint8Array) return src
  if (ArrayBuffer.isView(src)) return new Uint8Array(src.buffer, src.byteOffset, src.byteLength)
  return new Uint8Array(src)
}

/** Описание звука для MP4: AudioSpecificConfig, даже если браузер отдал ES_Descriptor или ничего */
export function aacDescription(desc: AllowSharedBufferSource | undefined, sampleRate: number, channels: number): Uint8Array {
  if (desc && desc.byteLength) {
    const b = bytes(desc)
    // ES_Descriptor начинается с тега 3; у AudioSpecificConfig в старших 5 битах тип AAC, 0 там не бывает
    if (b[0] !== 0x03) return b
    const inner = findDecSpecific(b, 0, b.length)
    if (inner?.length) return inner
  }
  return aacLcConfig(sampleRate, channels)
}
