class HpEnemyCounter {
    constructor() {
        this.numberDisplay = document.createElement("div");
        this.enemyOBJ = 0;
        this.enemyTimeout = null;
        this.observer = null;
        this.pointCounter = null;
        this.gameUpdateListener =  (event) => {
            if (event.data === "game-updated") {
                setTimeout(() => this.checkComp(), 500);
            }
        };


        window.glorpClient.settings.toggleHpEnemyCounter = (enabled) => this.toggle(enabled);
        this.toggle(true);
    }
    toggle(enabled) {
        window.glorpClient.newConsole.log("hpenemycounter", enabled);
        if (enabled) {
            window.chrome.webview.addEventListener("message", this.gameUpdateListener);
            this.checkComp();
        } else {
            removeEventListener('message', this.gameUpdateListener);
            this.numberDisplay.remove();
        }
    }
    
    processTeamScores = () => {
        document.querySelectorAll("#tScoreC1, #tScoreC2").forEach((team) => {
            if (team && !team.className.includes("you")) {
                const currentEnemyOBJ = parseInt(team.nextElementSibling.innerText);
                if (currentEnemyOBJ > this.enemyOBJ) {
                    this.pointCounter.innerText = (currentEnemyOBJ - this.enemyOBJ) / 10;
                    if (this.enemyTimeout) {
                        clearTimeout(this.enemyTimeout);
                    }
                    this.enemyTimeout = setTimeout(() => {
                        this.pointCounter.innerText = "0";
                        this.enemyTimeout = null;
                    }, 1600);
                }
                this.enemyOBJ = currentEnemyOBJ;
            }
        });
    }

    checkComp = () => {
        const gameStatus = window.getGameActivity();
        if (gameStatus.custom && gameStatus.mode == "Hardpoint") {
            if (this.gameUpdateListener) {
                window.chrome.webview.removeEventListener('message', this.gameUpdateListener);
            }
            this.setupDisplay();
        }
    }

    setupDisplay() {
        this.numberDisplay.classList.add("statIcon");
        this.numberDisplay.style.display = "inline-block";
        this.numberDisplay.style.backgroundColor = "transparent";
        this.numberDisplay.innerHTML = `
            <div class="greyInner" style="background-color:transparent">
                <span style="color:white;font-size:14px">on</span>
                <span id="myScoreVal" class="pointVal">0</span>
            </div>`;

        this.pointCounter = this.numberDisplay.querySelector(".pointVal");
        document.querySelector(".topRightCounters").appendChild(this.numberDisplay);

        this.observer = new MutationObserver(this.processTeamScores);
        this.observer.observe(document.querySelector("#teamScores"), {
            childList: true,
            subtree: true
        });
    }

}

new HpEnemyCounter()