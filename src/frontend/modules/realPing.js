class RealPing {
	constructor() {
		this.ingamePing = null;
		this.menuPing = null;
		this.originalTextContentDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "textContent");
		this.interval = null;
		this.listener = null;

		window.glorpClient.settings.toggleRealPing = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}

	applyPingDisplay(element) {
		if (!element) return;
		Object.defineProperty(element, "textContent", {
			set: () => {},
			get: () => this.originalTextContentDescriptor.get.call(element),
			configurable: true,
		});
	}

	async waitForElement(id, delay = 500, maxChecks = 30) {
		return new Promise((resolve) => {
			let currentChecks = 0;
			const checkElement = () => {
				const element = document.getElementById(id);
				if (element) {
					resolve(element);
					return;
				}

				currentChecks++;
				if (maxChecks && currentChecks >= maxChecks) {
					resolve(null);
					return;
				}

				setTimeout(checkElement, delay);
			};

			checkElement();
		});
	}

	async toggle(enabled) {
		if (enabled) {
			[this.ingamePing, this.menuPing] = await Promise.all([
				this.waitForElement("pingText"),
				this.waitForElement("menuPingText"),
			]);

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
			if (this.ingamePing) Object.defineProperty(this.ingamePing, "textContent", this.originalTextContentDescriptor);
			if (this.menuPing) Object.defineProperty(this.menuPing, "textContent", this.originalTextContentDescriptor);
		}
	}
}

new RealPing();
