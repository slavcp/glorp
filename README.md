![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/slavcp/glorp/total)<br>

Glorp uses unorthodox methods in attempt of fixing the issues modern chromium poses for a browser game

# Why is the client getting marked as a trojan?

- **Installer is not signed:** digital signatures help verify software, but in the case of a small open source project, paying for a license is not feasible, so antiviruses will mark it as malicious.

Review the source code if you have any doubts.

## Features

- [x] Raw input
- [x] DXGI hooks in an attempt of lowering latency
- [x] Light URL blocklist (customizable)
- [x] Resource swapper
- [x] Custom script support
- [x] Account Manager
- [x] Queue ranked externally
- [x] CPU Throttler
- [x] Autoupdater
- [x] Basic shortcuts (F11 - toggle fullscreen, F6 new lobby)
- [x] OBS game capture support via plugin
- [x] and more...

## Potential issues

If in a GPU bottleneck, the amount of frames displayed will drop severely, but the game's render loop won't slow down, this results in the client being almost unusable. <br>
Consider using the CPU Throttler in such scenario

## Building

- Prerequisites:
  - [Git LFS](https://git-lfs.com) `git lfs install`
  - [Rust & Cargo](https://rustup.rs/)
  - [Microsoft Visual C++](https://visualstudio.microsoft.com/downloads/)
  - [Node](https://nodejs.org/)
  - [pnpm](https://pnpm.io/installation)
  - [WiX 6 **(if packaging)**](https://github.com/wixtoolset/wix/releases)

1. `git clone https://github.com/slavcp/glorp.git`
2. `cd glorp`
3. `pnpm i`
4. `pnpm build`

## Credits

- [client-pp](https://github.com/6ct/clientpp)
- [crankshaft](https://github.com/KraXen72/crankshaft) - menu timer css
