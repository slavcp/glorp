import styles from "./components/base.css";
window.OffCliV = true;
window.canShowAds = false;
window.closeClient = () => window.chrome.webview.postMessage("close");
window.glorpClient = {};

(async () => {
  try {
    window.glorpClient.config = await new Promise((resolve) => {
      window.chrome.webview.addEventListener('message', (event) => {
        resolve(event.data);
      }, { once: true });
      window.chrome.webview.postMessage("getConfig");
    });
  } catch (e) {
    console.error('Failed to get config:', e);
  }
})();

/*
const newConsole = {
  log: console.log.bind(console),
} 
  */

document.addEventListener("DOMContentLoaded", () => {
  if (window.glorpClient?.config?.rawInput) {
    const originalRequestPointerLock = Element.prototype.requestPointerLock;
    Element.prototype.requestPointerLock = function (options = {}) {
      options.unadjustedMovement = true;
      return originalRequestPointerLock.call(this, options);
    }
  }
  if (window.glorpClient?.config?.exitButton) document.querySelector("#clientExit").style.display = "flex";
  if (window.glorpClient?.config?.hideBundles) {
    const bundlePopupObserver = new MutationObserver(() => {
      window.clearPops();
    }
    );

    bundlePopupObserver.observe(document.querySelector("#bundlePop"), { childList: true });
    setTimeout(() => bundlePopupObserver.disconnect(), 5000);
  };
  let baseCSS = document.createElement("style");
  baseCSS.innerHTML = styles;
  document.body.appendChild(baseCSS);
});

document.addEventListener("pointerlockchange", () => {
  window.chrome.webview.postMessage(`pointerLockChange,${document.pointerLockElement !== null}`);
})

Object.defineProperty(window, 'gameLoaded', {
  set(value) {
    if (value) {
      if (localStorage.getItem("firstLaunch") === null) {
        window.localStorage.setItem("firstLaunch", false);
        window.expertMode()
        window.windows[0].toggleType({ checked: true });
        window.selectScope(-1);
        window.selectReticle(-1);
        window.selectAttachment(-1);
        window.closWind();
      }
      import("./settingsMenu.js");
      if (window.glorpClient.config.compHPEnemyCounter) import("./hpEnemyCounter.js");
      if (window.glorpClient.config.accountManager) import("./accountManager.js");
      //import("./notifications.js").
    }
  },
});