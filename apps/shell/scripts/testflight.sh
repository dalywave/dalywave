#!/bin/bash
# Сборка для TestFlight: архив, подпись для App Store и загрузка в App Store Connect.
#   testflight.sh       — iPhone и iPad (схема dalywave-shell_iOS)
#   testflight.sh tv    — Apple TV (схема dalywave-tv): та же запись DalyWave (bundle id rest.daly.wave),
#                         платформа tvOS, та же версия MARKETING_VERSION, свой номер сборки
# EXPORT=local — только архив и подписанный файл в target/testflight/<сборка>, без загрузки (проверка).
# Ключ API — ~/.appstoreconnect/private_keys/AuthKey_<id>.p8 (роль Admin: сборка сама выпускает
# сертификат Apple Distribution и профили с Siri), его Key ID и Issuer ID —
# в ~/.appstoreconnect/dalywave.env (ASC_KEY_ID, ASC_ISSUER_ID). В репозиторий ничего из этого не идёт.
# Номер сборки — дата и время: растёт с каждой загрузкой, файлы проекта не меняются.
set -euo pipefail
cd "$(dirname "$0")/.."
ENV_FILE="$HOME/.appstoreconnect/dalywave.env"
[ -f "$ENV_FILE" ] && source "$ENV_FILE"
: "${ASC_KEY_ID:?нет ASC_KEY_ID (см. $ENV_FILE)}"
: "${ASC_ISSUER_ID:?нет ASC_ISSUER_ID (см. $ENV_FILE)}"
KEY="$HOME/.appstoreconnect/private_keys/AuthKey_$ASC_KEY_ID.p8"
[ -f "$KEY" ] || { echo "нет ключа $KEY"; exit 1; }

case "${1:-ios}" in
  ios) SCHEME=dalywave-shell_iOS; PLATFORM=iOS; NAME=DalyWave; SIGN=() ;;
  # Apple TV в команде нет — профиль разработки для подписи архива не выпустить. Архив собирается
  # без подписи, права (App Group для Top Shelf) ставятся временной подписью, а подписывает экспорт:
  # профиль App Store устройств не требует
  tv) SCHEME=dalywave-tv; PLATFORM=tvOS; NAME=DalyWaveTV; SIGN=(CODE_SIGNING_ALLOWED=NO) ;;
  *) echo "платформа: ios или tv"; exit 1 ;;
esac
case "${EXPORT:-upload}" in
  upload) DESTINATION=upload ;;
  local) DESTINATION=export ;;
  *) echo "EXPORT: upload или local"; exit 1 ;;
esac

# Сборка для App Store — клиент любого сервера: чистая установка начинается с экрана «Адрес сервера»
# (src/settings.rs). Уже вошедшие на устройстве (обновление) ничего не заметят. Сборка, сразу
# смотрящая на wave.daly.rest, как раньше: DALYWAVE_DEFAULT_SERVER=https://wave.daly.rest testflight.sh
export DALYWAVE_DEFAULT_SERVER="${DALYWAVE_DEFAULT_SERVER-none}"

BUILD="${BUILD_NUMBER:-$(date +%Y%m%d%H%M)}"
OUT="target/testflight/$BUILD-$PLATFORM"
mkdir -p "$OUT"
AUTH=(-allowProvisioningUpdates -authenticationKeyPath "$KEY"
  -authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID")

bash scripts/xcodegen.sh >/dev/null
echo "$PLATFORM, сборка $BUILD: архив"
set -o pipefail
xcodebuild -project gen/apple/dalywave-shell.xcodeproj -scheme "$SCHEME" -configuration release \
  -destination "generic/platform=$PLATFORM" -archivePath "$OUT/$NAME.xcarchive" \
  -derivedDataPath target/xcode-archive CURRENT_PROJECT_VERSION="$BUILD" ${SIGN[@]+"${SIGN[@]}"} "${AUTH[@]}" archive \
  | grep -E "error:|ARCHIVE (SUCCEEDED|FAILED)"
if [ "$PLATFORM" = tvOS ]; then
  APP="$OUT/$NAME.xcarchive/Products/Applications/$NAME.app"
  codesign -f -s - --entitlements gen/apple/TVTopShelf/DalyWaveTopShelf.entitlements \
    "$APP/PlugIns/DalyWaveTopShelf.appex"
  codesign -f -s - --entitlements gen/apple/TVApp/DalyWaveTV.entitlements "$APP"
fi

cat > "$OUT/ExportOptions.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>$DESTINATION</string>
  <key>teamID</key><string>Y5KR5JPCRG</string>
  <key>signingStyle</key><string>automatic</string>
  <key>manageAppVersionAndBuildNumber</key><false/>
  <key>uploadSymbols</key><true/>
</dict>
</plist>
EOF

if [ "$DESTINATION" = export ]; then
  echo "$PLATFORM, сборка $BUILD: подпись (без загрузки)"
else
  echo "$PLATFORM, сборка $BUILD: подпись и загрузка"
fi
xcodebuild -exportArchive -archivePath "$OUT/$NAME.xcarchive" -exportPath "$OUT" \
  -exportOptionsPlist "$OUT/ExportOptions.plist" "${AUTH[@]}" \
  | grep -E "error:|Upload|EXPORT (SUCCEEDED|FAILED)"
if [ "$DESTINATION" = export ]; then
  echo "Готово: $OUT (не загружено)"
else
  echo "Загружено: сборка $BUILD ($PLATFORM) появится в App Store Connect → TestFlight через 10–30 минут"
fi
