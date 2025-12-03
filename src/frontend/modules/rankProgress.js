class RankProgress {
	constructor() {
		this.ranks = [
			//shoutout mucnutty for elo values
			{ rank: "Unranked", elo: null, image: "https://assets.krunker.io/img/ranked/ranks/rank_unranked.svg" },
			{ rank: "Bronze 1", elo: 0, image: "https://assets.krunker.io/img/ranked/ranks/rank_bronze.svg" },
			{ rank: "Bronze 2", elo: 200, image: "https://assets.krunker.io/img/ranked/ranks/rank_bronze.svg" },
			{ rank: "Bronze 3", elo: 400, image: "https://assets.krunker.io/img/ranked/ranks/rank_bronze.svg" },
			{ rank: "Silver 1", elo: 700, image: "https://assets.krunker.io/img/ranked/ranks/rank_silver.svg" },
			{ rank: "Silver 2", elo: 900, image: "https://assets.krunker.io/img/ranked/ranks/rank_silver.svg" },
			{ rank: "Silver 3", elo: 1100, image: "https://assets.krunker.io/img/ranked/ranks/rank_silver.svg" },
			{ rank: "Gold 1", elo: 1300, image: "https://assets.krunker.io/img/ranked/ranks/rank_gold.svg" },
			{ rank: "Gold 2", elo: 1600, image: "https://assets.krunker.io/img/ranked/ranks/rank_gold.svg" },
			{ rank: "Gold 3", elo: 2000, image: "https://assets.krunker.io/img/ranked/ranks/rank_gold.svg" },
			{ rank: "Platinum", elo: 2300, image: "https://assets.krunker.io/img/ranked/ranks/rank_platinum.svg" },
			{ rank: "Diamond", elo: 3000, image: "https://assets.krunker.io/img/ranked/ranks/rank_diamond.svg" },
			{ rank: "Master", elo: 3300, image: "https://assets.krunker.io/img/ranked/ranks/rank_master.svg" },
			{ rank: "Kracked", elo: 4700, image: "https://assets.krunker.io/img/ranked/ranks/rank_kracked.svg" },
		];

		const existingBar = document.getElementById("glorp-elo-tracker");
		if (existingBar) existingBar.remove();
		const existingBtn = document.getElementById("glorp-rank-list-btn");
		if (existingBtn) existingBtn.remove();

		this.observer = new MutationObserver(() => this.checkForMenu());
		this.observer.observe(document.body, { childList: true, subtree: true });
	}

	checkForMenu() {
		const card = document.querySelector(".rank-card");
		const container = document.querySelector(".rank-and-stats");

		if (card && container) {
			if (!container.querySelector("#glorp-elo-tracker")) {
				this.injectBar(container);
			}
			if (!card.querySelector("#glorp-rank-list-btn")) {
				this.injectRankListButton(card);
			}
		}
	}

	getRankColor(rankName) {
		if (rankName.includes("Bronze")) return "#CD7F32";
		if (rankName.includes("Silver")) return "#C0C0C0";
		if (rankName.includes("Gold")) return "#FFD700";
		if (rankName.includes("Platinum")) return "#5E98D9";
		if (rankName.includes("Diamond")) return "#4B69FF";
		if (rankName.includes("Master")) return "#EE7032";
		if (rankName.includes("Kracked")) return "#FF0000";
		return "#ffffff";
	}

	injectRankListButton(card) {
		const btn = document.createElement("div");
		btn.id = "glorp-rank-list-btn";
		btn.className = "season-banner";
		btn.innerHTML = `<span class="material-icons" style="font-size:16px; vertical-align:middle; margin-right:4px;">list</span> Ranks`;
		btn.onclick = () => this.openPopup();

		if (window.getComputedStyle(card).position === 'static') {
			card.style.position = 'relative';
		}

		card.appendChild(btn);
	}

	openPopup() {
		if (document.getElementById("glorp-rank-overlay")) return;

		let gridItems = "";
		this.ranks.forEach(r => {
			const color = this.getRankColor(r.rank);
			gridItems += `
				<div class="rank-grid-item">
					<img src="${r.image}" loading="lazy">
					<div class="r-details">
						<div class="r-name" style="color: ${color}">${r.rank}</div>
						<div class="r-elo">${r.elo !== null ? r.elo + "+" : "Placement"}</div>
					</div>
				</div>
			`;
		});

		const overlay = document.createElement("div");
		overlay.id = "glorp-rank-overlay";
		overlay.onclick = (e) => {
			if (e.target.id === "glorp-rank-overlay") overlay.remove();
		};

		overlay.innerHTML = `
			<div class="rank-popup-content">
				<div class="rank-popup-header">
					<h2>Rank Distribution</h2>
					<div class="rank-popup-close" onclick="document.getElementById('glorp-rank-overlay').remove()">✕</div>
				</div>
				<div class="rank-grid-container">
					${gridItems}
				</div>
			</div>
		`;

		document.body.appendChild(overlay);
	}

	getRankData(currentElo) {
		const currentRankIndex = this.ranks.findLastIndex(r => r.elo !== null && currentElo >= r.elo);
		const currentRank = this.ranks[currentRankIndex];
		const nextRank = this.ranks[currentRankIndex + 1] || currentRank;
		const isMax = currentRankIndex === this.ranks.length - 1;

		let progress = 0;
		if (!isMax) {
			const range = nextRank.elo - currentRank.elo;
			const gained = currentElo - currentRank.elo;
			progress = (gained / range) * 100;
		} else {
			progress = 100;
		}

		return {
			currentRank,
			nextRank,
			progress: Math.min(Math.max(progress, 0), 100),
			isMax
		};
	}

	injectBar(container) {
		const statValues = container.querySelectorAll(".quick-stat-value");
		if (!statValues || statValues.length === 0) return;

		const currentElo = Number(statValues[0].textContent.replaceAll(",", ""));
		if (isNaN(currentElo)) return;

		const data = this.getRankData(currentElo);

		const wrapper = document.createElement("div");
		wrapper.id = "glorp-elo-tracker";

		const nextRankDisplay = data.isMax
			? ""
			: `<div class="rank-next-container">
				 <img src="${data.nextRank.image}" class="elo-rank-img">
				 <span>${data.nextRank.rank}</span>
			   </div>`;

		const barText = data.isMax ? `${currentElo} / MAX` : `${currentElo} / ${data.nextRank.elo}`;

		wrapper.innerHTML = `
			<div class="elo-info-row">
				<div class="rank-current-container">
					<img src="${data.currentRank.image}" class="elo-rank-img">
					<span>${data.currentRank.rank}</span>
				</div>
				<div class="elo-progress-bar-bg">
					<div class="elo-progress-bar-fill" style="width: ${data.progress}%"></div>
					<div class="elo-progress-text">${barText}</div>
				</div>
				${nextRankDisplay}
			</div>
		`;

		const statsBlock = container.querySelector(".quick-stats");
		if (statsBlock) container.insertBefore(wrapper, statsBlock);
		else container.appendChild(wrapper);
	}
}

new RankProgress();