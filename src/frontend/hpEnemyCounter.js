class HPEnemyCounter {
    constructor() {
        this.numberDisplay = document.createElement("div");
        this.enemyOBJ = 0;
        this.enemyTimeout = null;
        this.observer = null;
        this.pointCounter = null;
        this.gameUpdateListener = null;
        this.init();
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
        if (gameStatus.custom && gameStatus.mode === "Hardpoint") {
            window.chrome.webview.removeEventListener('message', this.gameUpdateListener);
            this.setupDisplay();
        }
    }

    setupDisplay() {
        if (this.gameUpdateListener) {
            this.gameUpdateListener.remove();
        }

        this.numberDisplay.classList.add("statIcon");
        this.numberDisplay.style.display = "block";
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

    init() {
        this.gameUpdateListener = window.chrome.webview.addEventListener('message', (event) => {
            if (event.data === 'game-updated') {
                setTimeout(() => this.checkComp(), 500);
            }
        });

        this.checkComp();
    }
}

new HPEnemyCounter()