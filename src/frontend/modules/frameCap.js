let frameCapValue = window.glorpClient.settings.data.frameCap;
let menuFrameCapValue = window.glorpClient.settings.data.menuFrameCap;

let fpsInterval;
let start = 0;
let rAF;

// requestAnimationFrame cant be adjusted after start
// so in case the user sets it to 0 the user will be wasting some performance every frame
// intermediary function attemps to lessen that by getting wiped when the cap is set to 0
const originalHookIntermediary = () => {
	// busy waiting sucks but it cant be done in another way without hooking the render loop
	while (window.performance.now() - start < fpsInterval) {}
	start = window.performance.now();
};

let hookIntermediary = originalHookIntermediary;

Object.defineProperty(window.glorpClient.settings.data, "frameCap", {
	set(value) {
		frameCapValue = value;
		setupFrameCap(frameCapValue);
	},
	get() {
		return frameCapValue;
	},
});

Object.defineProperty(window.glorpClient.settings.data, "menuFrameCap", {
	set(value) {
		menuFrameCapValue = value;
		setupFrameCap(menuFrameCapValue);
	},
	get() {
		return menuFrameCapValue;
	},
});

if (menuFrameCapValue === 0) hookIntermediary = () => {};
setupFrameCap(menuFrameCapValue);

window.glorpClient.settings.setupFrameCap = setupFrameCap;
function setupFrameCap(value) {
	if (value === 0) hookIntermediary = () => {};
	else hookIntermediary = originalHookIntermediary;

	fpsInterval = 1000 / (value * 1.03);
	if (!rAF) rAF = window.requestAnimationFrame;
	start = 0;
	window.requestAnimationFrame = (callback) => {
		hookIntermediary();
		rAF(callback);
	};
}
