import styles from "./components/base.css";

window.OffCliV = true;
window.closeClient = () => window.chrome.webview.postMessage("close");
window.glorpClient = {
  settings: {},
  newConsole: {
    log: console.log.bind(console),
  },
};

(async () => {
  try {
    window.glorpClient.settings.config = await new Promise((resolve) => {
      window.chrome.webview.addEventListener(
        "message",
        (event) => resolve(event.data),
        { once: true }
      );
      window.chrome.webview.postMessage("getConfig");
    });

    // check (start) if rpc is enabled
    const richPresenceEnabled = window.glorpClient.settings.config.rich_presence;
    window.chrome.webview.postMessage(`rpc-update,${richPresenceEnabled}`);
  } catch (e) {
    console.error("Failed to get config:", e);
  }
})();

document.addEventListener(
  "DOMContentLoaded",
  () => {
    let baseCSS = document.createElement("style");
    baseCSS.innerHTML = styles;
    document.body.appendChild(baseCSS);

    const originalAddEventListener =
      HTMLCanvasElement.prototype.addEventListener;
    HTMLCanvasElement.prototype.addEventListener = function (
      type,
      listener,
      options
    ) {
      if (type === "wheel")
        window.glorpClient.handleMouseWheel = (deltaY) =>
          listener(new WheelEvent("wheel", { deltaY }));
      return originalAddEventListener.call(this, type, listener, options);
    };

    if (window.glorpClient?.settings.config?.rawInput) {
      const originalRequestPointerLock =
        HTMLCanvasElement.prototype.requestPointerLock;
      HTMLCanvasElement.prototype.requestPointerLock = function (options) {
        return originalRequestPointerLock.call(this, {
          ...options,
          unadjustedMovement: true,
        });
      };
    }

    if (window.glorpClient?.settings.config?.exitButton)
      document.querySelector("#clientExit").style.display = "flex";

    if (window.glorpClient?.settings.config?.hideBundles) {
      const bundlePopupObserver = new MutationObserver(() =>
        window.clearPops()
      );

      bundlePopupObserver.observe(document.querySelector("#bundlePop"), {
        childList: true,
      });
      setTimeout(() => bundlePopupObserver.disconnect(), 20000);
    }
  },
  { once: true }
);

document.addEventListener("pointerlockchange", () => {
  window.chrome.webview.postMessage(
    `pointerLockChange,${document.pointerLockElement !== null}`
  );

  // safeguard
  setTimeout(
    () =>
      window.chrome.webview.postMessage(
        `pointerLockChange,${document.pointerLockElement !== null}`
      ),
    1000
  );
});

Object.defineProperty(window, "gameLoaded", {
  set(value) {
    if (value) {
      if (window.localStorage.getItem("firstLaunch") === null) {
        window.localStorage.setItem("kro_setngss_mouseAccel", false);
        window.localStorage.setItem("kro_setngss_mouseFlick", false);
        window.localStorage.setItem("firstLaunch", false);
        window.expertMode();
        window.windows[0].toggleType({ checked: true });
        window.selectScope(-1);
        window.selectReticle(-1);
        window.selectAttachment(-1);
        window.closWind();
      }

      import("./notifications.js").then(() => {
        // trick for hiding "PRESS ESC TO EXIT POINTER LOCK" also breaks the default notification for downloads
        const originalExportSettings = window.exportSettings;
        window.exportSettings = () => {
          window.glorpClient.showNotification(
            "Settings exported to Downloads!",
            false,
            3
          );
          return originalExportSettings();
        };
      });

      import("./settings.js").then(() => {
        if (window.glorpClient.settings.config.hpEnemyCounter)
          import("./modules/hpEnemyCounter.js");
        if (window.glorpClient.settings.config.accountManager)
          import("./modules/accountManager.js");
        if (window.glorpClient.settings.config.showPing)
          import("./modules/showPing.js");
        if (window.glorpClient.settings.config.menuTimer)
          import("./components/menuTimer.css").then((css) => {
            let menuTimerCSS = document.createElement("style");
            menuTimerCSS.id = "menuTimerCSS";
            menuTimerCSS.innerHTML = css.default;
            document.body.appendChild(menuTimerCSS);
          });
        // if (window.glorpClient.settings.config.autoSpectate)
        //   import("./modules/autoSpectate.js");
      });
    }
  },
});
