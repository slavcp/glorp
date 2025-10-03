const waitForElement = (selector) => {
	return new Promise((resolve) => {
		if (document.querySelector(selector)) return resolve(document.querySelector(selector));

		const observer = new MutationObserver(() => {
			if (document.querySelector(selector)) {
				resolve(document.querySelector(selector));
				observer.disconnect();
			}
		});
		observer.observe(document.body, { childList: true, subtree: true });
	});
};

const automateCompHost = async (params) => {
	window.openHostWindow(false, 1);
	window.originalConsole.log(params.mapId);
	const mapCheckbox = await waitForElement(`#${params.mapId}`);
	if (!mapCheckbox.checked) {
		mapCheckbox.click();
	}
	windows[7].switchTab(2);
	const team1Input = await waitForElement("#customSnameTeam1");
	team1Input.value = params.team1Name;
	const team2Input = await waitForElement("#customSnameTeam2");
	team2Input.value = params.team2Name;
	const teamSizeSelect = await waitForElement("#customStmSize");
	teamSizeSelect.value = params.teamSize;
	window.createPrivateRoom();
};
const args = window.glorpClient.launchArgs.split(" ");

for (const arg of args) {
	if (arg.includes("action=host-comp")) {
		const url = new URL(arg);
		automateCompHost(Object.fromEntries(url.searchParams.entries()));
	}
}
