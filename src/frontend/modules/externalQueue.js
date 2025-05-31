const externalQueue = document.createElement("button");
externalQueue.textContent = "open_in_new";
externalQueue.style = `background-color: #5ce05a;
    color: #ffffff;
    border-radius: 9px;
    padding: 20px 21px;
    font-family: 'Material Icons Outlined';
    cursor: pointer;
    transition: all 0.2s ease;
    font-size: 20px;
    text-shadow: 2px 2px 0px black !important;`;

externalQueue.onclick = openExtQueue;

const origRanked = window.openRankedMenu;
window.openRankedMenu = () => {
	origRanked.call();
	document.querySelector(".footer-controls").prepend(externalQueue);
};

function openExtQueue() {
	const queueWindow = window.open("about:blank", "_blank");

	queueWindow.token = localStorage.getItem("__FRVR_auth_access_token");
	queueWindow.maps = ["burg_new", "sandstorm_v3", "undergrowth", "industry", "site", "bureau"];
	const region = document.querySelector(".region-indicator").textContent.split(": ")[1];
	switch (region) {
		case "North America":
			queueWindow.region = "na";
			break;
		case "Europe":
			queueWindow.region = "eu";
			break;
		case "Asia":
			queueWindow.region = "as";
			break;
		default:
			alert("region not found");
			break;
	}

	const core = document.createElement("div");

	import("../components/queue/queue.html").then((html) => {
		core.innerHTML = html.default;
	});

	queueWindow.document.body.append(core);
}
