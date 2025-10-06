import styles from "./components/base.css";

let initialLoad = true;
window.OffCliV = true;
window.closeClient = () => window.chrome.webview.postMessage("close");
window.originalConsole = { ...window.console };

(async () => {
	window.glorpClient = await new Promise((resolve) => {
		window.chrome.webview.addEventListener("message", (event) => resolve(event.data), { once: true });
		window.chrome.webview.postMessage("get-info");
	});
})();

document.addEventListener(
	"DOMContentLoaded",
	() => {
		// load noticeable style changes and stuff that requires hooks earlier

		const baseCSS = document.createElement("style");
		baseCSS.innerHTML = styles;
		document.head.append(baseCSS);

		const originalAddEventListener = HTMLCanvasElement.prototype.addEventListener;
		HTMLCanvasElement.prototype.addEventListener = function (type, listener, options) {
			if (type === "wheel")
				window.glorpClient.handleMouseWheel = (deltaY) => listener(new WheelEvent("wheel", { deltaY }));
			return originalAddEventListener.call(this, type, listener, options);
		};

		if (window.glorpClient?.settings?.data?.cleanUI) {
			import("./components/clean.css").then((css) => {
				const cleanCSS = document.createElement("style");
				cleanCSS.id = "cleanUICSS";
				cleanCSS.innerHTML = css.default;
				document.head.append(cleanCSS);
			});
		}

		if (window.glorpClient?.settings?.data?.rawInput) {
			const originalRequestPointerLock = HTMLCanvasElement.prototype.requestPointerLock;
			HTMLCanvasElement.prototype.requestPointerLock = function (options) {
				return originalRequestPointerLock.call(this, {
					...options,
					unadjustedMovement: true,
				});
			};
		}

		if (window.glorpClient?.settings?.data?.exitButton) document.querySelector("#clientExit").style.display = "flex";
	},
	{ once: true },
);

document.addEventListener("pointerlockchange", () => {
	const pointerLock = document.pointerLockElement !== null;
	window.chrome.webview.postMessage(`pointer-lock, ${pointerLock}`);
});

Object.defineProperty(window, "gameLoaded", {
	set(value) {
		if (!value) return;
		window.chrome.webview.postMessage("game-updated");
		if (!initialLoad) return;
		if (sessionStorage.getItem("justLaunched") === null) sessionStorage.setItem("justLaunched", true);
		else sessionStorage.setItem("justLaunched", false);

		initialLoad = false;
		window.windows[0].toggleType({ checked: true });

		// append ranked and mod button to comp host ui
		document.querySelector("#compBtnLst").innerHTML += `
    <div class="compMenBtnS" onmouseenter='SOUND.play("tick_0",.1)' style="background-color: #f5479b" onclick="playSelect(),showWindow(4)"> <span class="material-icons" style="color:#fff;font-size:40px;vertical-align:middle;margin-bottom:12px">color_lens</span></div>
    <div class="compMenBtnS" onmouseenter='SOUND.play("tick_0",.1)' style="background-color: #5ce05a" onclick="playSelect(),window.openRankedMenu()"><span class="material-icons" style="color:#fff;font-size:40px;vertical-align:middle;margin-bottom:12px">star</span></div>`;

		import("./notifications.js");
		// trick for hiding "PRESS ESC TO EXIT POINTER LOCK" also breaks the default notification for downloads
		const originalExportSettings = window.exportSettings;
		window.exportSettings = () => {
			window.glorpClient.showNotification("Settings exported to Downloads!", false, 3);
			return originalExportSettings();
		};

		import("./settings.js");
		import("./modules/changelog.js");
		import("./modules/externalQueue.js");
		import("./modules/args.js");
		if (window.glorpClient?.settings.data?.hideBundles) window.bundlePopup = () => null;
		if (window.glorpClient?.settings.data?.hpEnemyCounter) import("./modules/hpEnemyCounter.js");
		if (window.glorpClient?.settings.data?.accountManager) import("./modules/accountManager.js");
		if (window.glorpClient?.settings.data?.showPing) import("./modules/showPing.js");
		if (window.glorpClient?.settings.data?.realPing) import("./modules/realPing.js");

		setTimeout(() => {
			if (sessionStorage.getItem("justLaunched") === "true" && window.glorpClient?.launchArgs)
				window.glorpClient.parseArgs(window.glorpClient.launchArgs);
		}, 2000);
		if (window.glorpClient?.settings.data?.autoSpec) {
			const trySetSpect = () => {
				const activity = window.getGameActivity();
				if (activity.map === null) {
					setTimeout(trySetSpect, 100);
					return;
				}
				if (!activity.custom) window.setSpect(true);
			};
			trySetSpect();
		}

		if (window.glorpClient?.settings.data?.discordRPC) {
			window.chrome.webview.addEventListener("message", (event) => {
				if (event.data !== "game-updated") return;
				setTimeout(() => {
					const gameStatus = window.getGameActivity();
					window.chrome.webview.postMessage(`rpc-update, ${gameStatus.mode},${gameStatus.map}`);
				}, 2000);
			});
		}

		if (window.glorpClient?.settings.data?.textSelect) {
			const textSelectCSS = document.createElement("style");
			textSelectCSS.id = "textSelectCSS";
			textSelectCSS.innerHTML = "#chatHolder * { user-select: text }";
			document.head.append(textSelectCSS);
		}

		if (window.glorpClient?.settings.data?.menuTimer) {
			const css = import("./components/menuTimer.css");
			const menuTimerCSS = document.createElement("style");
			menuTimerCSS.id = "menuTimerCSS";
			menuTimerCSS.innerHTML = css.default;
			document.head.append(menuTimerCSS);
		}
	},
});
