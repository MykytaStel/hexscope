#!/bin/sh
# Renders the link preview and the app icons from scripts/brand/*.html with
# headless Chrome, into apps/web/public.
#
#   sh scripts/make-brand-images.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
out="$here/../apps/web/public"
chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
shot() { "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
  --allow-file-access-from-files --window-size="$2" --screenshot="$out/$3" "file://$here/brand/$1" >/dev/null 2>&1; }
shot og.html 1200,630 og.png
# A JPEG half the size: some apps (WhatsApp) show no preview above ~300 KB.
sips -s format jpeg -s formatOptions 88 "$out/og.png" --out "$out/og.jpg" >/dev/null && rm "$out/og.png"
shot icon.html 512,512 icon-512.png
shot icon.html 512,512 icon-maskable-512.png
# Chrome will not render a window this small: scale the large one down.
sips -z 192 192 "$out/icon-512.png" --out "$out/icon-192.png" >/dev/null
ls -la "$out"/og.jpg "$out"/icon-*.png
