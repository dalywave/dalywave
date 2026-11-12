#!/bin/bash
# Сборка iPhone для TestFlight: архив, подпись для App Store и загрузка в App Store Connect.
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

BUILD="${BUILD_NUMBER:-$(date +%Y%m%d%H%M)}"
OUT="target/testflight/$BUILD"
mkdir -p "$OUT"
AUTH=(-allowProvisioningUpdates -authenticationKeyPath "$KEY"
  -authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID")

bash scripts/xcodegen.sh >/dev/null
echo "Сборка $BUILD: архив"
set -o pipefail
xcodebuild -project gen/apple/dalywave-shell.xcodeproj -scheme dalywave-shell_iOS -configuration release \
  -destination generic/platform=iOS -archivePath "$OUT/DalyWave.xcarchive" \
  -derivedDataPath target/xcode-archive CURRENT_PROJECT_VERSION="$BUILD" "${AUTH[@]}" archive \
  | grep -E "error:|ARCHIVE (SUCCEEDED|FAILED)"

cat > "$OUT/ExportOptions.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>upload</string>
  <key>teamID</key><string>Y5KR5JPCRG</string>
  <key>signingStyle</key><string>automatic</string>
  <key>manageAppVersionAndBuildNumber</key><false/>
  <key>uploadSymbols</key><true/>
</dict>
</plist>
EOF

echo "Сборка $BUILD: подпись и загрузка"
xcodebuild -exportArchive -archivePath "$OUT/DalyWave.xcarchive" -exportPath "$OUT" \
  -exportOptionsPlist "$OUT/ExportOptions.plist" "${AUTH[@]}" \
  | grep -E "error:|Upload|EXPORT (SUCCEEDED|FAILED)"
echo "Загружено: сборка $BUILD появится в App Store Connect → TestFlight через 10–30 минут"
