class RealPing {
	constructor() {
		this.ingamePing = null;
		this.menuPing = null;
		this.originalTextContent = null;

		window.glorpClient.settings.toggleRealPing = (enabled) => this.toggle(enabled);
		this.toggle(true);

		setInterval(() => {
			window.chrome.webview.postMessage("ping");
		}, 5000);

		window.chrome.webview.addEventListener("message", (event) => {
			window.originalConsole.log(event);
			if (!event.data.pingInfo) return;
			window.glorpClient.settings.ping = Number.parseInt(event.data.pingInfo);
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

	setRealPing(element) {
		if (!element) return;

		this.originalTextContent = Object.getOwnPropertyDescriptor(Element.prototype, "textContent");

		Object.defineProperty(element, "textContent", {
			set: (e) => {
				if (window.glorpClient.settings.ping) element.innerText = window.glorpClient.settings.ping;
				else element.innerText = 0;
			},
			get: () => element.innerText,
		});
	}

	restoreOriginalProperty(element) {
		if (!element || !this.originalTextContent) return;
		Object.defineProperty(element, "textContent", this.originalTextContent);
	}

	async toggle(enabled) {
		if (enabled) {
			const [ingamePing, menuPing] = await Promise.all([
				this.waitForElement("pingText"),
				this.waitForElement("menuPingText"),
			]);
			this.ingamePing = ingamePing;
			this.menuPing = menuPing;

			if (ingamePing) this.setRealPing(ingamePing);
			if (menuPing) this.setRealPing(menuPing);
		} else {
			if (this.ingamePing) this.restoreOriginalProperty(this.ingamePing);
			if (this.menuPing) this.restoreOriginalProperty(this.menuPing);
		}
	}
}

new RealPing();
