![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/slavcp/glorp/total)<br>

Glorp uses unorthodox methods in attempt of fixing the issues modern chromium poses for a browser game

# Why is the client getting marked as a trojan?

- **The installer is not signed:** digital signatures help verify software, but in the case of such a small open source project, paying for a license is not feasible, so antiviruses will mark it as malicious.
- **DLL Injection:** Glorp utilizes DLL injection, a technique used for running code in the context of another application's space, something that is often used for malicious purposes. I assure you that the client only uses this for the user's convenience.

I strongly urge you to **review the source code** if you have any doubts.

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
- [x] CPU Throttler
- [x] and more...

## Potential issues

If in a GPU bottleneck, the amount of frames displayed will drop severely, but the game render loop won't slow down this causes the client to be almost unusable. <br>
consider using the CPU throttler in such scenario

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
- [nsis-tauri-utils](https://github.com/tauri-apps/nsis-tauri-utils) - using for installer
- [crankshaft](https://github.com/KraXen72/crankshaft) - menu timer css
