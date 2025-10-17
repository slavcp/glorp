import cSettings from "../cSettings.json";

const debounceTimers = {};

window.glorpClient.settings.changeSetting = (id, value, slider) => {
	if (value === "") return;
	document.querySelector(`#${id}`).value = value;

	if (typeof value === "string") {
		const numberRegex = /^-?\d*\.?\d+$/;
		if (numberRegex.test(value)) {
			if (value.includes(".")) value = Number.parseFloat(value);
			else value = Number.parseInt(value);
		}
	}

	if (document.querySelector(`#slid_input_${id}`) && slider) {
		document.querySelector(`#slid_input_${id}`).value = value;

		if (debounceTimers[id]) clearTimeout(debounceTimers[id]);

		debounceTimers[id] = setTimeout(() => {
			window.glorpClient.settings.data[id] = value;
			window.chrome.webview.postMessage(`set-config, ${id}, ${value}`);

			delete debounceTimers[id];
		}, 500);
		return;
	}

	switch (id) {
		case "exitButton":
			document.querySelector("#clientExit").style.display = `${value ? "flex" : "none"}`;
			break;
		case "menuTimer":
			if (value) {
				import("./components/menuTimer.css").then((css) => {
					const menuTimerCSS = document.createElement("style");
					menuTimerCSS.id = "menuTimerCSS";
					menuTimerCSS.innerHTML = css.default;
					document.head.append(menuTimerCSS);
				});
			} else {
				document.querySelector("#menuTimerCSS")?.remove();
			}
			break;
		case "cleanUI": {
			if (value) {
				import("./components/clean.css").then((css) => {
					const cleanCSS = document.createElement("style");
					cleanCSS.id = "cleanCSS";
					cleanCSS.innerHTML = css.default;
					document.head.append(cleanCSS);
				});
			} else {
				document.querySelector("#cleanCSS")?.remove();
			}
			break;
		}
		case "textSelect": {
			if (value) {
				const textSelectCSS = document.createElement("style");
				textSelectCSS.id = "textSelect";
				textSelectCSS.innerHTML = "#chatHolder * { user-select: text }";
				document.head.append(textSelectCSS);
			} else {
				document.querySelector("#textSelect")?.remove();
			}
			break;
		}
	}

	const toggleFunctionName = `toggle${id.charAt(0).toUpperCase() + id.slice(1)}`;
	if (typeof window.glorpClient.settings[toggleFunctionName] !== "function") {
		try {
			import(`./modules/${id}.js`);
		} catch {
			/*  */
		}
	} else {
		window.glorpClient.settings[toggleFunctionName](value);
	}

	window.glorpClient.settings.data[id] = value;
	window.chrome.webview.postMessage(`set-config, ${id}, ${value}`);
};

class SettingsManager {
	constructor() {
		this.settingsWindow = window.windows[0];
		this.init();
	}
	init() {
		const origGetSettings = this.settingsWindow.getSettings;
		this.settingsWindow.getSettings = (...args) =>
			origGetSettings.call(this.settingsWindow, ...args).replace(/^<\/div>/, "") + this.getCSettings();

		this.settingsWindow.getCSettings = () => this.getCSettings();
	}

	searchMatches(setting) {
		const query = this.settingsWindow.settingSearch.toLowerCase() || "";
		return (setting.name.toLowerCase() || "").includes(query) || (setting.category.toLowerCase() || "").includes(query);
	}

	generateHtml(option) {
		const value = window.glorpClient.settings.data[option.id];
		switch (option.type) {
			// biome-ignore format: it looks like straight ass when biome formats it
			case "checkbox":
				return `<label class='switch'>
                    <input id="${option.id}" type='checkbox'
                           onclick='window.glorpClient.settings.changeSetting("${option.id}", this.checked, false)'
                           ${value ? "checked" : ""}>
                    <span class='slider'></span>
                </label>
                ${option.button
						? `<div class="settingsBtn" style="margin-right: 20px; width: auto" onclick="${option.buttonAction}">${option.button}</div>`
						: ""
					}`;
			// biome-ignore format: see above
			case "slider":
				return `<input type="number" class="sliderVal" id="slid_input_${option.id}" min="${option.min}" value="${value || option.min}"
                step="${option.step}" oninput='window.glorpClient.settings.changeSetting("${option.id}", this.value, true)' style="margin-right:0px;border-width:0px">
                <div class="slidecontainer" style="margin-top: -8px;"><input type="range" id="${option.id}" min="${option.min}" max="${option.max}"
				step="${option.step}" value="${value}" class="sliderM" oninput='window.glorpClient.settings.changeSetting("${option.id}", this.value, true)'></div>`;
			// biome-ignore format: see above
			case "select":
				return `<select id="${option.id}" class="inputGrey2" onchange='window.glorpClient.settings.changeSetting("${option.id}", this.value, false)'>
                    ${option.options.map(
					(opt) => `<option value="${opt}" ${opt === value ? "selected" : ""}>${opt}</option>`,
				)}
                    </select>`;
			case "none":
				return option.button
					? `<div class="settingsBtn" style="margin-right: 20px; width: auto" onclick="${option.buttonAction}">${option.button}</div>`
					: "";
		}
	}

	getCSettings() {
		if (
			this.settingsWindow.tabs.advanced.length !== this.settingsWindow.tabIndex + 1 &&
			!this.settingsWindow.settingSearch
		)
			return "";

		let tempHTML = "<div class='glorpSettings'>";
		let previousCategory = null;

		for (const entry of Object.keys(cSettings)) {
			const setting = cSettings[entry];
			setting.html = this.generateHtml(setting);

			if (this.settingsWindow.settingSearch && !this.searchMatches(setting)) continue;

			if (previousCategory !== setting.category) {
				if (previousCategory) tempHTML += "</div>";

				previousCategory = setting.category;
				tempHTML += `<div class='setHed' id="setHed_glorpClient_${setting.category}" onclick='window.windows[0].collapseFolder(this)'>
                    <span class='material-icons plusOrMinus'>keyboard_arrow_down</span> ${setting.category}</div>
                    <div class='setBodH' id="setBod_glorpClient_${setting.category}">`;
			}

			tempHTML += `<div class='settName' ${setting.description ? `title="${setting.description}"` : ""}>
                ${setting.name}
                ${setting.needsRestart ? ' <span style="color: #eb5656" title="Requires Restart">*</span>' : ""}
                ${setting.html}</div>`;
		}

		return tempHTML ? `${tempHTML}</div></div></div>` : "";
	}
}

new SettingsManager();
