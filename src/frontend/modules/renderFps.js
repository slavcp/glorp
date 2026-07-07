class RenderFps {
	constructor() {
		this.ingameFPS = null;
		this.menuFPS = null;
		this.listener = null;

		window.glorp.settings.toggleRenderStats = (enabled) => this.toggle(enabled);
		window.glorp.settings.toggleFpsMonitor = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}

	applyFpsDisplay(element) {
		if (!element) return;
		Object.defineProperty(element, "textContent", { set: () => {}, configurable: true });
	}

	async toggle(enabled) {
		[this.ingameFPS, this.menuFPS] = await Promise.all([
			waitForElement("#ingameFPS"),
			waitForElement("#menuFPS"),
		]);

		if (enabled) {
			this.applyFpsDisplay(this.ingameFPS);
			this.applyFpsDisplay(this.menuFPS);
			this.listener = (event) => {
				if (event.data.fpsInfo === undefined) return;
				this.ingameFPS.innerText = event.data.fpsInfo;
				this.menuFPS.innerText = event.data.fpsInfo;
			};
			window.chrome.webview.addEventListener("message", this.listener);
		} else {
			if (this.listener) {
				window.chrome.webview.removeEventListener("message", this.listener);
				this.listener = null;
			}
			delete this.ingameFPS.textContent;
			delete this.menuFPS.textContent;
			this.ingameFPS.removeAttribute("title");
			this.menuFPS.removeAttribute("title");
		}
	}
}

new RenderFps();
