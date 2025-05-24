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
			window.chrome.webview.addEventListener("message", (event) => resolve(event.data), { once: true });
			window.chrome.webview.postMessage("getConfig");
		});
	} catch (e) {
		console.error("Failed to get config:", e);
	}
})();

document.addEventListener(
	"DOMContentLoaded",
	() => {
		const baseCSS = document.createElement("style");
		baseCSS.innerHTML = styles;
		document.head.appendChild(baseCSS);
		window.localStorage.setItem("cont_shoot1Key_alt", "131");

		if (window.glorpClient?.settings.config?.cleanUI) {
			import("./components/clean.css").then((css) => {
				const cleanCSS = document.createElement("style");
				cleanCSS.id = "cleanCSS";
				cleanCSS.innerHTML = css.default;
				document.head.appendChild(cleanCSS);
			});
		}

		const originalAddEventListener = HTMLCanvasElement.prototype.addEventListener;
		HTMLCanvasElement.prototype.addEventListener = function (type, listener, options) {
			if (type === "wheel")
				window.glorpClient.handleMouseWheel = (deltaY) => listener(new WheelEvent("wheel", { deltaY }));
			return originalAddEventListener.call(this, type, listener, options);
		};

		if (window.glorpClient?.settings.config?.rawInput) {
			const originalRequestPointerLock = HTMLCanvasElement.prototype.requestPointerLock;
			HTMLCanvasElement.prototype.requestPointerLock = function (options) {
				return originalRequestPointerLock.call(this, {
					...options,
					unadjustedMovement: true,
				});
			};
		}

		if (window.glorpClient?.settings.config?.exitButton) document.querySelector("#clientExit").style.display = "flex";
	},
	{ once: true },
);

document.addEventListener("pointerlockchange", () => {
	window.chrome.webview.postMessage(`pointerLock,${document.pointerLockElement !== null}`);

	// safeguard
	setTimeout(() => window.chrome.webview.postMessage(`pointerLock,${document.pointerLockElement !== null}`), 1000);
});

Object.defineProperty(window, "gameLoaded", {
	set(value) {
		if (!value) return;
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

		// binds shoot to f20
		setTimeout(() => {
			window.changeContSet();
			window.changeCont("shoot", 1, undefined);
			document.dispatchEvent(new KeyboardEvent("keydown", { keyCode: 131, bubbles: true }));
			document.dispatchEvent(new KeyboardEvent("keyup", { keyCode: 131, bubbles: true }));
			window.closWind();
		}, 1400);

		(async () => {
			await import("./notifications.js");
			// trick for hiding "PRESS ESC TO EXIT POINTER LOCK" also breaks the default notification for downloads
			const originalExportSettings = window.exportSettings;
			window.exportSettings = async () => {
				window.glorpClient.showNotification("Settings exported to Downloads!", false, 3);
				return originalExportSettings();
			};

			await import("./settings.js");
			await import("./modules/changelog.js");
			if (window.glorpClient?.settings.config?.hideBundles) window.bundlePopup = () => null;
			if (window.glorpClient?.settings.config?.hpEnemyCounter) await import("./modules/hpEnemyCounter.js");
			if (window.glorpClient?.settings.config?.accountManager) await import("./modules/accountManager.js");
			if (window.glorpClient?.settings.config?.showPing) await import("./modules/showPing.js");

			if (window.glorpClient?.settings.config?.autoSpec) {
				const trySetSpect = async () => {
					const activity = window.getGameActivity();
					if (activity.map === null) {
						await new Promise((resolve) => setTimeout(resolve, 100));
						return trySetSpect();
					}
					if (!activity.custom) window.setSpect(true);
				};
				await trySetSpect();
			}

			if (window.glorpClient?.settings.config?.discordRPC) {
				window.chrome.webview.addEventListener("message", async (event) => {
					if (event.data !== "game-updated") return;
					await new Promise((resolve) => setTimeout(resolve, 2000));
					const gameStatus = window.getGameActivity();
					window.window.chrome.webview.postMessage(`rpcUpdate,${gameStatus.mode},${gameStatus.map}`);
				});
			}

			if (window.glorpClient?.settings.config?.textSelect) {
				const textSelectCSS = document.createElement("style");
				textSelectCSS.id = "textSelect";
				textSelectCSS.innerHTML = "#chatHolder * { user-select: text }";
				document.head.appendChild(textSelectCSS);
			}

			if (window.glorpClient?.settings.config?.menuTimer) {
				const css = await import("./components/menuTimer.css");
				const menuTimerCSS = document.createElement("style");
				menuTimerCSS.id = "menuTimerCSS";
				menuTimerCSS.innerHTML = css.default;
				document.head.appendChild(menuTimerCSS);
			}
		})();
	},
});
