class RenderFps {
	constructor() {
		this.ingameFPS = null;
		this.menuFPS = null;
		this.listener = null;
		this.fps = null;
		this.elementObservers = new Map();
		this.domObserver = null;

		window.glorp.settings.toggleRenderStats = (enabled) => this.toggle(enabled);
		window.glorp.settings.toggleFpsMonitor = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}

	applyFpsDisplay(selector) {
		const element = document.querySelector(selector);
		if (!element) return;
		const existing = this.elementObservers.get(selector);
		if (existing?.element === element) {
			existing.render();
			return;
		}
		existing?.observer.disconnect();

		const render = () => {
			if (this.fps === null || element.textContent === String(this.fps)) return;
			element.textContent = this.fps;
		};

		const observer = new MutationObserver(render);
		observer.observe(element, { childList: true, characterData: true, subtree: true });
		this.elementObservers.set(selector, { element, observer, render });
		render();
	}

	renderFps() {
		this.applyFpsDisplay("#ingameFPS");
		this.applyFpsDisplay("#menuFPS");
	}

	async toggle(enabled) {
		[this.ingameFPS, this.menuFPS] = await Promise.all([waitForElement("#ingameFPS"), waitForElement("#menuFPS")]);

		if (enabled) {
			this.renderFps();
			this.domObserver = new MutationObserver(() => this.renderFps());
			this.domObserver.observe(document.documentElement, { childList: true, subtree: true });
			this.listener = (event) => {
				if (event.data.fpsInfo === undefined) return;
				this.fps = event.data.fpsInfo;
				this.renderFps();
			};
			window.chrome.webview.addEventListener("message", this.listener);
		} else {
			if (this.listener) {
				window.chrome.webview.removeEventListener("message", this.listener);
				this.listener = null;
			}
			this.domObserver?.disconnect();
			this.domObserver = null;
			for (const { observer } of this.elementObservers.values()) observer.disconnect();
			this.elementObservers.clear();
			this.fps = null;
			this.ingameFPS.removeAttribute("title");
			this.menuFPS.removeAttribute("title");
		}
	}
}

new RenderFps();
