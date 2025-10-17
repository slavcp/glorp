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
	window.originalConsole.log(`Test ${params.mapId}`);

	await waitForElement(".hostTb0");

	let mapCheckbox = null;

	mapCheckbox = document.querySelector(`#${params.mapId}`);

	if (!mapCheckbox) {
		const allMapNameElements = document.querySelectorAll(".hostMap .hostMapName");
		const targetNameElement = Array.from(allMapNameElements).find(
			(el) => el.innerText.trim().toLowerCase() === params.mapId.toLowerCase(),
		);
		if (targetNameElement) {
			mapCheckbox = targetNameElement.parentElement.querySelector('input[type="checkbox"]');
		}
	}

	if (!mapCheckbox) return;

	if (!mapCheckbox.checked) {
		mapCheckbox.click();
	}
	windows[7].switchTab(2);

	const team1Input = await waitForElement("#customSnameTeam1");
	team1Input.value = params.team1Name;
	const team2Input = await waitForElement("#customSnameTeam2");
	team2Input.value = params.team2Name;

	const teamSizeSelect = await waitForElement("#customStmSize");

	const teamSizeMap = {
		"1v1": "0",
		"2v2": "1",
		"3v3": "2",
		"4v4": "3",
	};

	const finalTeamSize = teamSizeMap[params.teamSize] || params.teamSize;
	teamSizeSelect.value = finalTeamSize;
	if (params.webhook) {
		try {
			const webhookInput = await waitForElement("#customSwebhook");
			webhookInput.value = decodeURIComponent(params.webhook);
		} catch {
			/* */
		}
	}
	window.createPrivateRoom();
};

window.glorpClient.parseArgs = (args) => {
	args = args.split(" ");
	for (const arg of args) {
		if (arg.includes("action=host-comp")) {
			const url = new URL(arg);
			automateCompHost(Object.fromEntries(url.searchParams.entries()));
		}
	}
};

window.chrome.webview.addEventListener("message", (event) => {
	if (!event.data.args) return;
	window.glorpClient.parseArgs(event.data.args);
});
