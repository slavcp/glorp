import cSettings from "../cSettings.json";

window.glorpClient.settings.changeSetting = (id, value, fromSlider) => {

if (document.querySelector(`#input_${id}`) && fromSlider) {
    document.querySelector(`#input_${id}`).value = value;   
} else {
    document.querySelector(`#${id}`).value = value;   
}



if (id == "exitButton") {
        document.querySelector("#clientExit").style.display = `${value? "flex" : "none"}`;
}

        const toggleFunctionName = `toggle${id.charAt(0).toUpperCase() + id.slice(1)}`;
        if (typeof window.glorpClient.settings[toggleFunctionName] !== 'function') {
            try {
                import(`./modules/${id}.js`);
            } catch {
                /*  */
            }
        } else {
            window.glorpClient.settings[toggleFunctionName](value);
        }

    

    window.glorpClient.settings.config[id] = value;
    window.chrome.webview.postMessage(`setConfig,${id},${value}`);
}

class SettingsManager {
    constructor() {
        this.settingsWindow = window.windows[0];
        this.init();
    }
    init() {
        const origGetSettings = this.settingsWindow.getSettings;
        this.settingsWindow.getSettings = (...args) =>
            origGetSettings.call(this.settingsWindow, ...args).replace(/^<\/div>/, "") +
            this.getCSettings();

        this.settingsWindow.getCSettings = () => this.getCSettings();
    }

    searchMatches(setting) {
        const query = this.settingsWindow.settingSearch.toLowerCase() || "";
        return (
            (setting.name.toLowerCase() || "").includes(query) ||
            (setting.category.toLowerCase() || "").includes(query)
        );
    }

    generateHtml(option) {
        const value = window.glorpClient.settings.config[option.id];
        switch (option.type) {
            case "checkbox":
                return `<label class='switch'>
                    <input id="${option.id}" type='checkbox'
                           onclick='window.glorpClient.settings.changeSetting("${option.id}", this.checked, false)'
                           ${value ? "checked" : ""}>
                    <span class='slider'></span>
                </label>
                ${option.button ? `<div class="settingsBtn" style="margin-right: 20px; width: auto" onclick="${option.buttonAction}">${option.button}</div>` : ""}`;
            case "slider": 
                return `<input type="number" class="sliderVal" id="input_${option.id}" min="1" max="5" value="${value || 1}"step="${option.step}" oninput='window.glorpClient.settings.changeSetting("${option.id}", this.value, false)' style="margin-right:0px;border-width:0px">
                <div class="slidecontainer" style="margin-top: -8px;"><input type="range" id="${option.id}" min="${option.min}" max="${option.max}" step="${option.step}" value="${value}" class="sliderM" oninput='window.glorpClient.settings.changeSetting("${option.id}", this.value, true)'></div>`;
            case "none":
                return ""
        }
    }

    getCSettings() {
        if (this.settingsWindow.tabs.advanced.length !== this.settingsWindow.tabIndex + 1 &&
            !this.settingsWindow.settingSearch) {
            return "";
        }

        let tempHTML = "";
        let previousCategory = null;

        Object.keys(cSettings).forEach((entry) => {
            const setting = cSettings[entry];
            setting.html = this.generateHtml(setting);

            if (this.settingsWindow.settingSearch && !this.searchMatches(setting)) return;

            if (previousCategory !== setting.category) {
                if (previousCategory) tempHTML += "</div>";

                previousCategory = setting.category;
                tempHTML += `<div class='setHed' id='setHed_glorpClient' onclick='window.windows[0].collapseFolder(this)'>
                    <span class='material-icons plusOrMinus'>keyboard_arrow_down</span>${setting.category}</div>
                    <div class='setBodH' id='setBod_glorpClient'>`;
            }

            tempHTML += `<div class='settName' ${setting.description ? `title="${setting.description}"` : ""}>
                ${setting.name}
                ${setting.needsRestart ? ' <span style="color: #eb5656" title="Requires Restart">*</span>' : ""} 
                ${setting.html}</div>`;
        });

        return tempHTML ? tempHTML + "</div></div>" : "";
    }

}

new SettingsManager();
