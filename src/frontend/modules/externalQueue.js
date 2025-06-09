let queueWindow;
const externalQueue = document.createElement("button");

if (window.glorpClient?.launchArgs?.includes("glorp://ranked") && sessionStorage.getItem("justLaunched") === "true") {
	window.openRankedMenu();
	const startButton = document.querySelector(".start-button");

	const observer = new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			if (!startButton) return;
			if (mutation.attributeName === "disabled" && !startButton.disabled) {
				observer.disconnect();

				const startTime = Date.now();
				// most of the time the game doesnt log you in in time
				const checkSigned = () => {
					if (Date.now() - startTime > 5000) return;
					const signedIn = document.querySelector("#signedInHeaderBar").style.display !== "none";
					if (!signedIn) setTimeout(checkSigned, 100);
					else startButton.click();
				};
				checkSigned();
			}
		}
	});
	observer.observe(startButton, { attributes: true });
}

externalQueue.textContent = "open_in_new";
externalQueue.style = `background-color: #5ce05a;
    color: #ffffff;
    border-radius: 9px;
    padding: 20px 21px;
    font-family: 'Material Icons Outlined';
    cursor: pointer;
    font-size: 20px;
    text-shadow: 2px 2px 0px black !important;`;

externalQueue.onclick = openExtQueue;

const origRanked = window.openRankedMenu;
window.openRankedMenu = () => {
	origRanked.call();
	document.querySelector(".footer-controls").prepend(externalQueue);
};

function openExtQueue() {
	const screenWidth = window.screen.width;
	const screenHeight = window.screen.height;
	const windowWidth = screenWidth * 0.35;
	const windowHeight = windowWidth * 0.4;
	const menuBarHeight = 150;
	const left = (screenWidth - windowWidth) / 2;
	const top = (screenHeight - windowHeight - menuBarHeight) / 2;
	const queueWindow = window.open(
		"about:blank",
		"_blank",
		`width=${windowWidth},height=${windowHeight},left=${left},top=${top}`,
	);

	let region = document.querySelector(".region-indicator").textContent.split(": ")[1];
	switch (region) {
		case "North America":
			region = "na";
			break;
		case "Europe":
			region = "eu";
			break;
		case "Asia":
			region = "as";
			break;
	}
	let token = localStorage.getItem("__FRVR_auth_access_token");
	token = token.replace(/"/g, "");
	token = token.replace("/", "");

	queueWindow.info = {
		token: token,
		region: region,
	};

	import("../components/queue/index.html").then((html) => {
		const parser = new DOMParser();
		const doc = parser.parseFromString(html.default, "text/html");
		for (const child of doc.body.children) {
			queueWindow.document.body.appendChild(child.cloneNode(true));
		}
		for (const child of doc.head.children) {
			if (child.tagName === "SCRIPT") {
				const script = document.createElement("script");
				script.textContent = child.textContent;
				queueWindow.document.head.appendChild(script);
			} else {
				queueWindow.document.head.appendChild(child.cloneNode(true));
			}
		}
	});
}
