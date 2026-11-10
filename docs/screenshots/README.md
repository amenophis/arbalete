# Screenshots

The README's screenshots, taken from a development build on demo data only: public sites, no
account, nothing from a real profile.

## Taking them again

1. Write the demo data, light or dark. The icons folder is optional: one `<host>.png` per site
   (`en.wikipedia.org.png`…), which the tabs show before they load, such as the sites'
   `/favicon.ico` converted with `sips -s format png`.

   ```sh
   cargo run -p arbalete-core --example screenshot-demo -- /tmp/demo/data /tmp/demo/home light /tmp/demo/icons
   ```

2. Run a copy on it, in English, apart from your own data:

   ```sh
   mise run app
   open -n "target/bundle/Arbalete Dev.app" --env ARBALETE_DATA_DIR=/tmp/demo/data \
     --env ARBALETE_HOME=/tmp/demo/home --env ARBALETE_DEBUG_PORT=9445 \
     --args -AppleLanguages "(en-US)" -AppleLocale en_US --lang=en-US
   ```

   The pages follow the system's appearance. For light pages on a dark system, keep
   `node docs/screenshots/scheme.mjs 9445 light` running.

3. Capture the window, 1280 × 800 points, without its shadow (`-o`), by its id:

   ```sh
   screencapture -o -x -l <window id> hero-light.png
   ```

   An extension's popup is a window of its own: capture it too and lay it over the main one.
   The Spaces animation records the window alone, so that nothing over it shows:
   `swiftc -O -o record docs/screenshots/record.swift`, then `./record <window id> 7 frames`
   (JPEG frames at the screen's pixel size, 24 a second) while the Spaces menu's Space 2, 3
   and 1 switch Spaces.
   The import screen is a first launch: an empty data folder with the demo home, whose Arc,
   Chrome and Firefox profiles hold no data.

4. Scale to 1600 pixels wide and encode as WebP:

   ```sh
   ffmpeg -i hero-light.png -vf scale=1600:-1:flags=lanczos hero-1600.png
   cwebp -q 82 -m 6 -sharp_yuv hero-1600.png -o hero-light.webp
   ```

   The animation drops the frames that repeat the one before and keeps how long each shows, so
   that it stays sharp and light:

   ```sh
   ffmpeg -framerate 24 -pattern_type glob -i 'frames/*.jpg' \
     -vf scale=1600:-1:flags=lanczos,mpdecimate,showinfo -fps_mode vfr kept/%03d.png 2> kept.log
   ```

   then `img2webp -loop 0 -lossy -q 80 -m 6`, each kept frame after `-d` and its duration in
   milliseconds, the time to the next one's `pts_time` in `kept.log`:

   ```sh
   img2webp -loop 0 -lossy -q 80 -m 6 -d 1500 kept/001.png -d 42 kept/002.png … -o spaces.webp
   ```

The extensions shot needs Bitwarden and uBlock Origin Lite installed from the Chrome Web Store
in the copy; the demo data pins them by the address.
