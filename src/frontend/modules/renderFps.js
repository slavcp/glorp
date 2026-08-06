class RenderFps {
	constructor() {
		this.ingameFPS = null;
		this.menuFPS = null;
		this.listener = null;
		this.gameFPS = null;
		window.glorp.settings.toggleRenderFps = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}

	applyFpsDisplay(element) {
		if (!element) return;
		Object.defineProperty(element, "textContent", {
			set: (value) => {
				this.gameFPS = value;
			},
			configurable: true,
		});
	}

	async toggle(enabled) {
		[this.ingameFPS, this.menuFPS] = await Promise.all([waitForElement("#ingameFPS"), waitForElement("#menuFPS")]);

		if (enabled) {
			this.applyFpsDisplay(this.ingameFPS);
			this.applyFpsDisplay(this.menuFPS);

			this.listener = (event) => {
				console.log(event);
				const fps = event.data?.fpsInfo;
				if (fps === undefined) return;

				if (this.ingameFPS) this.ingameFPS.innerText = `${this.gameFPS} ${fps}`;
				if (this.menuFPS) this.menuFPS.innerText = `${this.gameFPS} ${fps}`;
			};

			window.chrome.webview.addEventListener("message", this.listener);
		} else {
			if (this.listener) {
				window.chrome.webview.removeEventListener("message", this.listener);
				this.listener = null;
			}
			if (this.ingameFPS) delete this.ingameFPS.textContent;
			if (this.menuFPS) delete this.menuFPS.textContent;
		}
	}
}

new RenderFps();
