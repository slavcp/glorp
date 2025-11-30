window.checkPointerLock(); // enable the cpu throttle after the game loads

// trick for hiding "PRESS ESC TO EXIT POINTER LOCK" also breaks the default notification for downloads
const originalExportSettings = window.exportSettings;
window.exportSettings = () => {
	window.glorpClient.showNotification("Settings exported to Downloads!", false, 3);
	return originalExportSettings();
};

// disable cpu throttling while on the skins menu
// fixes it taking 10 years to load

const originalshowWindow = window.showWindow;
window.showWindow = (...args) => {
	const number = args[0];
	switch (number) {
		case 3:
		case 53:
			window.chrome.webview.postMessage("pointer-lock, true");
			break;
		case 15:
		case 26:
		case 52:
		case 9:
		case 44:
		case 43:
		case 40:
		case 38:
		case 50:
		case 17:
		case 39:
		case 51:
		case 16:
		case 34:
			window.chrome.webview.postMessage("pointer-lock, false");
			break;
	}
	return originalshowWindow.apply(this, args);
};

const originalclosWind = windowclosWind;
window.closWind = (...args) => {
	window.chrome.webview.postMessage("pointer-lock, false");
	return originalclosWind.apply(this, args);
};
