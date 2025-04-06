glorp is a krunker client that uses dll injection to improve your gaming experience <br>
_it will trigger your antivirus_

for some people it works wonders, for some it lags like crazy

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
