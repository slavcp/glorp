glorp is a krunker client that uses dll injection in an attempt to fix the issues normal clients face, <br>

glorp is the #1 client by a margin for about 43.14% of the playerbase!!
the other 56.86% will expect:

- network issues
- stuttering
- audio glitches
  and anything inbetween 😀😀

## Features

- [x] **Proper** Raw input
- [x] Increased performance
- [x] Hook DXGI parameters in an attempt of lowering latency
- [x] Optimized URL blocklist (only ~40 entries, customizable)
- [x] Lightweight - bundle size ~3mb
- [x] Resource swapper
- [x] Custom script support
- [x] Account Manager
- [x] Lightweight autoupdater

## Building

- Prerequisites:
  - [Rust **Nightly** & Cargo](https://rustup.rs/)
  - [Microsoft Visual C++](https://visualstudio.microsoft.com/downloads/)
  - [Node](https://nodejs.org/)
  - [pnpm](https://pnpm.io/installation)
  - [NSIS **(if packaging)**](https://nsis.sourceforge.io/)

1. `git clone https://github.com/slavcp/glorp.git`
2. `cd glorp`
3. `pnpm i`
4. `pnpm build`

## Credits

- [client-pp](https://github.com/6ct/clientpp)
- [nsis-tauri-utils](https://github.com/tauri-apps/nsis-tauri-utils)
