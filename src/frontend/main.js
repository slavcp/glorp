import styles from "./components/base.css";
let firstLoad = true;
let safeguardTimeout;
window.OffCliV = true;
window.closeClient = () => window.chrome.webview.postMessage("close");
window.glorpClient = {
	settings: {},
	console: {
		log: console.log.bind(console),
	},
};

(async () => {
	window.glorpClient = await new Promise((resolve) => {
		window.chrome.webview.addEventListener("message", (event) => resolve(event.data), { once: true });
		window.chrome.webview.postMessage("getInfo");
	});
})();

document.addEventListener(
	"DOMContentLoaded",
	() => {
		const baseCSS = document.createElement("style");
		baseCSS.innerHTML = styles;
		document.head.append(baseCSS);
		window.localStorage.setItem("cont_shoot1Key_alt", "131");

		if (window.glorpClient?.settings.data?.cleanUI) {
			import("./components/clean.css").then((css) => {
				const cleanCSS = document.createElement("style");
				cleanCSS.id = "cleanCSS";
				cleanCSS.innerHTML = css.default;
				document.head.append(cleanCSS);
			});
		}

		const originalAddEventListener = HTMLCanvasElement.prototype.addEventListener;
		HTMLCanvasElement.prototype.addEventListener = function (type, listener, options) {
			if (type === "wheel")
				window.glorpClient.handleMouseWheel = (deltaY) => listener(new WheelEvent("wheel", { deltaY }));
			return originalAddEventListener.call(this, type, listener, options);
		};

		if (window.glorpClient?.settings.data?.rawInput) {
			const originalRequestPointerLock = HTMLCanvasElement.prototype.requestPointerLock;
			HTMLCanvasElement.prototype.requestPointerLock = function (options) {
				return originalRequestPointerLock.call(this, {
					...options,
					unadjustedMovement: true,
				});
			};
		}

		if (window.glorpClient?.settings.data?.exitButton) document.querySelector("#clientExit").style.display = "flex";
	},
	{ once: true },
);

document.addEventListener("pointerlockchange", () => {
	window.chrome.webview.postMessage(`pointerLock,${document.pointerLockElement !== null}`);
	clearTimeout(safeguardTimeout);
	safeguardTimeout = setTimeout(
		() => window.chrome.webview.postMessage(`pointerLock,${document.pointerLockElement !== null}`),
		1000,
	);
});

function bindF20() {
	window.changeContSet();
	window.changeCont("shoot", 1, undefined);
	document.dispatchEvent(new KeyboardEvent("keydown", { keyCode: 131, bubbles: true }));
	document.dispatchEvent(new KeyboardEvent("keyup", { keyCode: 131, bubbles: true }));
	window.closWind();
}

Object.defineProperty(window, "gameLoaded", {
	set(value) {
		if (!value) return;
		window.chrome.webview.postMessage("game-updated");
		if (!firstLoad) return;
		const justLaunched = sessionStorage.getItem("justLaunched");
		if (justLaunched === null) sessionStorage.setItem("justLaunched", true);
		else sessionStorage.setItem("justLaunched", false);

		firstLoad = false;
		window.windows[0].toggleType({ checked: true });

		// binds shoot to f20
		setTimeout(() => bindF20(), 1400);

		// append ranked and mod button to comp host ui
		document.querySelector("#compBtnLst").innerHTML += `
    <div class="compMenBtnS" 
        onmouseenter='SOUND.play("tick_0",.1)' 
		style="background-color: #f5479b"
        onclick="playSelect(),showWindow(4)">
        <span class="material-icons" 
            style="color:#fff;font-size:40px;vertical-align:middle;margin-bottom:12px">
            color_lens
        </span>
    </div>
    <div class="compMenBtnS"
        onmouseenter='SOUND.play("tick_0",.1)'
		style="background-color: #5ce05a"
        onclick="playSelect(),window.openRankedMenu()">
        <span class="material-icons"
            style="color:#fff;font-size:40px;vertical-align:middle;margin-bottom:12px">
            star
        </span>
    </div>`;

		// avoid not being able to shoot when importing settings
		const originalImportSettings = window.importSettings;
		window.importSettings = () => {
			originalImportSettings.call();
			bindF20();
		};

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
			await import("./modules/externalQueue.js");
			if (window.glorpClient?.settings.data?.hideBundles) window.bundlePopup = () => null;
			if (window.glorpClient?.settings.data?.hpEnemyCounter) await import("./modules/hpEnemyCounter.js");
			if (window.glorpClient?.settings.data?.accountManager) await import("./modules/accountManager.js");
			if (window.glorpClient?.settings.data?.showPing) await import("./modules/showPing.js");
			if (window.glorpClient?.settings.data?.realPing) await import("./modules/realPing.js");

			if (window.glorpClient?.settings.data?.autoSpec) {
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

			if (window.glorpClient?.settings.data?.discordRPC) {
				window.chrome.webview.addEventListener("message", async (event) => {
					if (event.data !== "game-updated") return;
					await new Promise((resolve) => setTimeout(resolve, 2000));
					const gameStatus = window.getGameActivity();
					window.window.chrome.webview.postMessage(`rpcUpdate,${gameStatus.mode},${gameStatus.map}`);
				});
			}

			if (window.glorpClient?.settings.data?.textSelect) {
				const textSelectCSS = document.createElement("style");
				textSelectCSS.id = "textSelect";
				textSelectCSS.innerHTML = "#chatHolder * { user-select: text }";
				document.head.append(textSelectCSS);
			}

			if (window.glorpClient?.settings.data?.menuTimer) {
				const css = await import("./components/menuTimer.css");
				const menuTimerCSS = document.createElement("style");
				menuTimerCSS.id = "menuTimerCSS";
				menuTimerCSS.innerHTML = css.default;
				document.head.append(menuTimerCSS);
			}
		})();
	},
});
