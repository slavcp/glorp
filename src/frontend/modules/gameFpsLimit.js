const nativeRAF = window.requestAnimationFrame;
let nextFrameTime = performance.now();

window.requestAnimationFrame = function (callback) {
	return nativeRAF(function (timestamp) {
		const targetFps = window.glorp?.settings?.data?.gameFpsLimit ?? 0;

		if (targetFps > 0) {
			let targetInterval;
			if (targetFps > 200) targetInterval = 1000 / (targetFps * 1.0055);
			else targetInterval = 1000 / targetFps;

			while (performance.now() < nextFrameTime) {}

			const now = performance.now();

			if (now - nextFrameTime > targetInterval) {
				nextFrameTime = now + targetInterval;
			} else nextFrameTime += targetInterval;
		} else nextFrameTime = performance.now();

		callback(timestamp);
	});
};
