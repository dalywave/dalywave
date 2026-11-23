# Third-party models and programs in the DalyWave ml image

The DalyWave code in this image is licensed under AGPL-3.0. The image also contains model files and a
program made by others, under their own licenses, listed below. In the image this folder is
`/usr/share/doc/dalywave-ml/`; the models are in `/app/models`, with exact file checksums in
`/app/models/manifest.lock.json` (also served at `GET /v1/models`). Download URLs and revisions are
pinned in `src/dalywave_ml/manifest.py`.

## Not in the image: MTG models (non-commercial use only)

Discogs-EffNet (`discogs-effnet-bsdynamic-1`) and its classifier heads (mood_happy, mood_sad,
mood_aggressive, mood_relaxed, mood_party, danceability, voice_instrumental,
approachability_regression, engagement_regression) by the Music Technology Group, Universitat Pompeu
Fabra, are licensed CC BY-NC-SA 4.0 (https://creativecommons.org/licenses/by-nc-sa/4.0/) — for
non-commercial use only. They are **not distributed with this image**: on first start the service
downloads them unmodified from https://essentia.upf.edu/models/ into `ML_MTG_MODELS_DIR` (`/models`,
best kept on a volume), checks their sha256 and writes `ATTRIBUTION.md` next to them with the authors,
sources and license. Commercial use needs a separate license from MTG:
https://www.upf.edu/web/mtg/contact.

## CLAP (larger CLAP, music)

- `clap/audio_model.onnx`, `clap/text_model.onnx`: from the checkpoint `music_audioset_epoch_15_esc_90.14.pt`
  by LAION (https://github.com/LAION-AI/CLAP, https://huggingface.co/lukewys/laion_clap, revision
  `b3708341862f581175dba5c356a4ebf74a9b6651`), CC0-1.0. **Modified**: the audio and text encoders are
  exported to ONNX with weights stored in float16 by `export/export_clap.py`.
- `clap/tokenizer.json`: from https://huggingface.co/laion/larger_clap_music (revision
  `a0b4534a14f58e20944452dff00a22a06ce629d1`), Apache-2.0 — `Apache-2.0.txt`. Unmodified.

## Beat This!

- `beat_this/beat_this.onnx`, `beat_this/mel-filterbank.bin`: checkpoint `final0` of Beat This! by the
  Institute of Computational Perception, JKU Linz (https://github.com/CPJKU/beat_this), MIT —
  `beat_this-MIT.txt`. ONNX conversion by musetric (https://huggingface.co/musetric/beat-this-onnx,
  revision `4e971bd43753023e1bf961c34a0cb74985cfcb88`), MIT. Unmodified.

## S-KEY

- `skey/skey.onnx`, `skey/config.json`: S-KEY by Deezer (https://github.com/deezer/skey), MIT —
  `skey-MIT.txt`. ONNX conversion by musetric (https://huggingface.co/musetric/skey-onnx, revision
  `9d90d2a9ff6679df1d64000f4fa750643f247643`), MIT. Unmodified.

## FFmpeg

- `/usr/local/bin/ffmpeg`: FFmpeg 8.1.3 built from the source release
  https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz (sha256
  `7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3`) with LGPL components only
  (audio demuxers and decoders, no network, no GPL parts), LGPL-2.1-or-later — `LGPL-2.1.txt`.
  Build options: `services/ml/Dockerfile` in the DalyWave repository.
