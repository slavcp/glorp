class RealPing {
	constructor() {
		this.ingamePing = null;
		this.menuPing = null;
		this.interval = null;
		this.listener = null;

		window.glorp.settings.toggleRealPing = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}

	applyPingDisplay(element) {
		if (!element) return;
		Object.defineProperty(element, "textContent", {
			set: () => {},
			configurable: true,
		});
	}

	async toggle(enabled) {
		[this.ingamePing, this.menuPing] = await Promise.all([
			waitForElement("#pingText"),
			waitForElement("#menuPingText"),
		]);
		if (enabled) {
			this.applyPingDisplay(this.ingamePing);
			this.applyPingDisplay(this.menuPing);
			this.interval = setInterval(() => {
				window.chrome.webview.postMessage("ping");
			}, 3000);

			this.listener = window.chrome.webview.addEventListener("message", (event) => {
				if (!event.data.pingInfo) return;
				this.ingamePing.innerText = event.data.pingInfo;
				this.menuPing.innerText = event.data.pingInfo;
			});
		} else {
			clearInterval(this.interval);
			window.chrome.webview.removeEventListener("message", this.listener);
			delete this.ingamePing.textContent;
			delete this.menuPing.textContent;
		}
	}
}

new RealPing();
