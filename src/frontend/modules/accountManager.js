class AccountManager {
	constructor() {
		this.button = document.createElement("div");
		this.button.textContent = "Accounts";
		this.button.classList.add("button", "buttonB", "bigShadowT");
		this.button.style.cssText =
			"display: block; padding-top: 7px; padding-bottom: 22px; font-size: 25px!important; padding-bottom: 22px; margin-top: 7px; height: 21px; line-height: 35px; width: 162px; font-size:20px!important; margin-left: 3px;";

		this.container = document.createElement("div");
		this.accounts = JSON.parse(localStorage.getItem("accounts") || "[]");

		window.glorpClient.settings.toggleAccountManager = (enabled) => this.toggle(enabled);

		this.toggle(true);
	}

	gameUpdateListener = (event) => {
		if (event.data === "game-updated") setTimeout(() => this.checkComp(), 2000);
	};

	toggle(enabled) {
		if (enabled) {
			window.chrome.webview.addEventListener("message", this.gameUpdateListener);
			document.querySelector("#signedOutHeaderBar")?.append(this.button);
			this.button.addEventListener("click", this.createMenu);
			this.checkComp();
		} else {
			window.chrome.webview.removeEventListener("message", this.gameUpdateListener);
			this.button.removeEventListener("click", this.createMenu);
			this.button.remove();
		}
	}

	handleMenuClick = (event) => {
		const clickedElement = event.target;
		if (clickedElement.classList.contains("accountHolder")) {
			this.handleAccountSelection(clickedElement);
		} else {
			switch (clickedElement.id) {
				case "newAccountButton":
					this.switchTabs();
					break;
				case "createAccountButton":
					this.createNewAccount();
					break;
				case "accountMenu":
				case "windowHolder":
				case "accountContainer":
					this.removeWindow();
					break;
			}
		}
	};

	encode(decoded) {
		const key = decoded.length;
		const encoded = decoded
			.split("")
			.map((char) => String.fromCharCode(char.charCodeAt(0) + key))
			.join("");
		return encodeURIComponent(encoded);
	}

	createNewAccount() {
		let username = document.querySelector("#username").value;
		let password = document.querySelector("#password").value;
		const color = document.querySelector("#color-picker").value;

		if (username.replace(/\s/, "") === "" || password.replace(/\s/, "") === "") {
			this.switchTabs();
			return;
		}
		if (this.accounts.some((account) => this.decode(account.username) === username)) return;

		username = this.encode(username);
		password = this.encode(password);

		this.accounts.push({ username, password, color });
		localStorage.setItem("accounts", JSON.stringify(this.accounts));
		this.resetForm();
		this.updateAccounts();
		this.switchTabs();
	}

	decode(encoded) {
		const username = decodeURIComponent(encoded);
		const key = username.length;
		return username
			.split("")
			.map((char) => String.fromCharCode(char.charCodeAt(0) - key))
			.join("");
	}

	handleAccountSelection(element) {
		const account = this.accounts.find((acc) => this.decode(acc.username) === element.textContent);

		this.removeWindow();
		window.loginOrRegister();

		setTimeout(() => {
			if (document.querySelector(".auth-toggle-btn").textContent.includes("username"))
				document.querySelector(".auth-toggle-btn").click();

			setTimeout(() => {
				const nameInput = document.querySelector("#accName");
				const passInput = document.querySelector("#accPass");
				nameInput.value = this.decode(account.username);
				passInput.value = this.decode(account.password);
				// send input otherwise it thinks its empty
				nameInput.dispatchEvent(new Event("input", { bubbles: true }));
				passInput.dispatchEvent(new Event("input", { bubbles: true }));
				document.querySelector(".io-button").click();
			}, 1);
		}, 1);
	}

	resetForm() {
		document.querySelector("#color-picker").value = `#${Math.floor(Math.random() * 16777215)
			.toString(16)
			.padStart(6, "0")}`;
		document.querySelector("#username").value = "";
		document.querySelector("#password").value = "";
	}

	switchTabs() {
		document.querySelector("#accountContainerTab").classList.toggle("hidden");
		document.querySelector("#accountCreatorTab").classList.toggle("hidden");
	}

	updateAccounts() {
		const accountContainer = document.querySelector("#accountContainer");
		while (accountContainer.children.length > 0) {
			accountContainer.removeChild(accountContainer.children[0]);
		}

		for (const account of this.accounts) {
			const accountHolder = document.createElement("div");
			accountHolder.classList.add("accountHolder");
			accountHolder.style.color = account.color;
			accountHolder.textContent = this.decode(account.username);
			accountContainer.append(accountHolder);
		}
	}

	removeWindow() {
		this.container.removeEventListener("contextmenu", this.handleMenuClick);
		document.removeEventListener("click", this.handleMenuClick);
		this.container.remove();
	}

	createMenu = () => {
		import("../components/accountManager.html").then((html) => {
			this.container.innerHTML = html.default;
			document.body.append(this.container);
			this.updateAccounts();
			this.container.addEventListener("contextmenu", this.removeAccount);
			document.addEventListener("click", this.handleMenuClick);
			document.querySelector("#color-picker").value = `#${Math.floor(Math.random() * 16777215)
				.toString(16)
				.padStart(6, "0")}`;
		});
	};

	removeAccount = (event) => {
		event.preventDefault();
		const clickedElement = event.target;
		if (clickedElement.classList.contains("accountHolder")) {
			const index = this.accounts.findIndex((account) => this.decode(account.username) === clickedElement.textContent);
			if (index > -1) {
				this.accounts.splice(index, 1);
				localStorage.setItem("accounts", JSON.stringify(this.accounts));
				this.updateAccounts();
			}
		}
	};

	checkComp() {
		if (document.querySelector(".cmpTmHed")) {
			if (this.gameUpdateListener) window.chrome.webview.removeEventListener("message", this.gameUpdateListener);

			this.button.style.cssText =
				"display: block; padding: 14px 24px 22px; bottom: 0; right: 0; z-index: 9; font-size: 21px !important; position: absolute;";
			document.querySelector("#compBtnLst").append(this.button);
		}
	}
}

new AccountManager();
