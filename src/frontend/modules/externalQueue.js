const externalQueue = document.createElement("button");

externalQueue.textContent = "open_in_new";
externalQueue.style = /* css */ `background-color: #5ce05a;
    color: #ffffff;
    border-radius: 9px;
    padding: 20px 21px;
    font-family: 'Material Icons Outlined';
    cursor: pointer;
    font-size: 20px;
    text-shadow: 2px 2px 0px black !important;
	margin-left: 2px;`;

externalQueue.onclick = openExtQueue;

const origRanked = window.openRankedMenu;
window.openRankedMenu = () => {
	origRanked.call();
	const footer = document.querySelector(".footer-controls");
	const lastChild = footer.lastElementChild;
	footer.insertBefore(externalQueue, lastChild);
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
	const allRegions = localStorage.getItem("s_rankedAllRegions") === "true";
	queueWindow.info = {
		allRegions: allRegions,
		token: token,
		region: region,
	};

	import("../components/queue/index.html").then((html) => {
		const parser = new DOMParser();
		const doc = parser.parseFromString(html.default, "text/html");
		for (const child of doc.body.children) queueWindow.document.body.appendChild(child.cloneNode(true));

		for (const child of doc.head.children) {
			if (child.tagName === "SCRIPT") {
				const script = document.createElement("script");
				script.textContent = child.textContent;
				queueWindow.document.head.appendChild(script);
			} else queueWindow.document.head.appendChild(child.cloneNode(true));
		}
	});
}
