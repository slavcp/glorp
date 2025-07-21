let frameCapValue = window.glorpClient.settings.data.frameCap;
let menuFrameCapValue = window.glorpClient.settings.data.menuFrameCap;

let fpsInterval;

// requestAnimationFrame cant be adjusted after start
// so in case the user sets it to 0 the user will be wasting some performance every frame
// intermediary function attemps to lessen that by getting wiped when the cap is set to 0

Object.defineProperty(window.glorpClient.settings.data, "frameCap", {
	set(value) {
		frameCapValue = value;
	},
	get() {
		return frameCapValue;
	},
});

Object.defineProperty(window.glorpClient.settings.data, "menuFrameCap", {
	set(value) {
		menuFrameCapValue = value;
	},
	get() {
		return menuFrameCapValue;
	},
});

window.glorpClient.settings.setFrameCap = setFrameCap;
function setFrameCap(value) {
	if (value === 0) {
		fpsInterval = 0;
		return;
	}
	fpsInterval = 1000 / (value * 1);
}

setFrameCap(menuFrameCapValue);

let start = window.performance.now();
const rAF = window.requestAnimationFrame;
window.requestAnimationFrame = (callback) => {
	while (window.performance.now() - start < fpsInterval) {}
	start = window.performance.now();
	rAF(callback);
};
