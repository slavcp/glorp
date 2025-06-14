class RealPing {
	constructor() {
		this.ingamePing = null;
		this.menuPing = null;
		this.originalTextContent = null;
		this.multiplier = 1.7;

		window.glorpClient.settings.toggleRealPing = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}

	// stole this from pc7 LOL
	// - aashten
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
			set: (value) => {
				const numValue = Number(value);
				if (!Number.isNaN(numValue)) {
					const multiplied = Math.round(numValue * this.multiplier);
					element.innerText = multiplied;
				} else {
					element.innerText = value;
				}
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
