const nativeRAF = window.requestAnimationFrame;
let nextFrameTime = performance.now();

window.requestAnimationFrame = function (callback) {
	return nativeRAF(function loop(timestamp) {
		const targetFps = window.glorp?.settings?.data?.gameFpsLimit ?? 0;

		if (targetFps > 0) {
			let targetInterval;
			if (targetFps > 200) targetInterval = 1000 / (targetFps * 1.0055);
			else targetInterval = 1000 / targetFps;

			// re-arm instead of busy-wait to keep main thread free between frames when RAF is uncapped
			const now = performance.now();
			if (now < nextFrameTime) {
				nativeRAF(loop);
				return;
			}

			if (now - nextFrameTime > targetInterval) {
				nextFrameTime = now + targetInterval;
			} else nextFrameTime += targetInterval;
		} else nextFrameTime = performance.now();

		callback(timestamp);
	});
};
