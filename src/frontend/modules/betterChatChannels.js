class BetterChat {
	constructor() {
		this.teamModes = new Set([
			"Team Deathmatch",
			"Hardpoint",
			"Capture the Flag",
			"Hide & Seek",
			"Infected",
			"Last Man Standing",
			"Simon Says",
			"Prop Hunt",
			"Boss Hunt",
			"Deposit",
			"Stalker",
			"Kill Confirmed",
			"Defuse",
			"Traitor",
			"Blitz",
			"Domination",
			"Squad Deathmatch",
			"Team Defender",
		]);
		this.styles = document.createElement("style");
		import("../components/betterChat.css").then((css) => {
			this.styles.innerHTML = css.default;
		});

		window.glorpClient.settings.toggleBetterChat = (enabled) => this.toggle(enabled);
		this.initDom();
		this.observer = new MutationObserver((mutations) => this.parseMessages(mutations));
		this.toggle(true);
	}

	initDom() {
		this.chatHolder = document.querySelector("#chatHolder");
		this.chatList = document.querySelector("#chatList");
		this.chatInput = document.querySelector("#chatInput");
		this.chatSwitch = document.querySelector("#chatSwitch");
		this.channelT = document.createElement("div");
		this.channelA = document.createElement("div");
		this.channelT.style.cssText = "float: left; display: inline-block; margin-right: 5px; color: #eb5656;";
		this.channelT.textContent = "[TEAM]";
		this.channelA.style.cssText = "float: left; display: inline-block; margin-right: 5px; color: #9eeb56;";
		this.channelA.textContent = "[MATCH]";
	}

	switchChat = (event) => {
		if (event.key !== "Tab") return;
		window.switchChat(this.chatSwitch);
		event.preventDefault();
	};

	parseMessages(mutations) {
		for (const mutation of mutations) {
			for (const node of mutation.addedNodes) {
				const chatItem = node.querySelector(".chatItem");
				const chatMsg = chatItem.querySelector(".chatMsg");
				if (
					chatMsg.textContent.includes("on a \u200E") ||
					chatMsg.textContent.includes("\u200E unboxed") ||
					!this.teamModes.has(window.getGameActivity().mode) ||
					!node.dataset.tab
				)
					continue;
				if (chatMsg.style.color === "rgb(252, 3, 236)" || chatMsg.style.color === "rgb(0, 255, 0)") {
					if (chatMsg.textContent.includes("Text & Voice Chat")) node.remove();
					continue;
				}
				if (node.dataset.tab === "0") {
					const clone = this.channelA.cloneNode(true);
					chatMsg.insertBefore(clone, chatMsg.firstChild);
				}
				if (node.dataset.tab === "1") {
					const clone = this.channelT.cloneNode(true);
					chatMsg.insertBefore(clone, chatMsg.firstChild);
				}
			}
		}
	}

	toggle(enabled) {
		if (enabled) {
			document.head.append(this.styles);
			this.chatInput.addEventListener("keydown", this.switchChat, { capture: true });
			this.observer.observe(this.chatList, { childList: true });
		} else {
			this.styles.remove();
			this.chatInput.removeEventListener("keydown", this.switchChat, { capture: true });
			this.observer.disconnect();
		}
	}
}

new BetterChat();
