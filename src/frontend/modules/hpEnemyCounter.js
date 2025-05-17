class HpEnemyCounter {
	constructor() {
		this.numberDisplay = document.createElement("div");
		this.numberDisplay.id = "hpEnemyCounter";
		this.numberDisplay.classList.add("statIcon");
		this.numberDisplay.style.cssText = "inline-block; transform: translate(0, -2.7px);";
		this.numberDisplay.innerHTML = `
            <div class="greyInner" style="display: flex">
                <span style="color:white; font-size:15px; margin-right: 4px;">on</span>
                <span id="myScoreVal" class="pointVal">0</span>
            </div>`;

		this.enemyOBJ = 0;
		this.enemyTimeout = null;
		this.observer = null;
		this.pointCounter = null;
		this.gameUpdateListener = (event) => {
			if (event.data === "game-updated") {
				setTimeout(() => this.checkComp(), 2000);
			}
		};

		window.glorpClient.settings.toggleHpEnemyCounter = (enabled) => this.toggle(enabled);
		this.toggle(true);
	}
	toggle(enabled) {
		if (enabled) {
			window.chrome.webview.addEventListener("message", this.gameUpdateListener);
			this.checkComp();
		} else {
			removeEventListener("message", this.gameUpdateListener);
			this.numberDisplay.remove();
		}
	}

	processTeamScores = () => {
		for (const team of document.querySelectorAll("#tScoreC1, #tScoreC2")) {
			if (team && !team.className.includes("you")) {
				const currentEnemyOBJ = Number.parseInt(team.nextElementSibling.innerText);
				if (currentEnemyOBJ > this.enemyOBJ) {
					this.pointCounter.innerText = (currentEnemyOBJ - this.enemyOBJ) / 10;
					if (this.enemyTimeout) clearTimeout(this.enemyTimeout);
					this.enemyTimeout = setTimeout(() => {
						this.pointCounter.innerText = "0";
						this.enemyTimeout = null;
					}, 1600);
				}
				this.enemyOBJ = currentEnemyOBJ;
			}
		}
	};

	checkComp = () => {
		if (document.querySelector("#compClassPHolder")) {
			if (this.gameUpdateListener) window.chrome.webview.removeEventListener("message", this.gameUpdateListener);

			this.setupDisplay();
		}
	};

	setupDisplay() {
		this.pointCounter = this.numberDisplay.querySelector(".pointVal");
		document.querySelector(".topRightCounters").appendChild(this.numberDisplay);

		this.observer = new MutationObserver(this.processTeamScores);
		this.observer.observe(document.querySelector("#teamScores"), {
			childList: true,
			subtree: true,
		});
	}
}

new HpEnemyCounter();
