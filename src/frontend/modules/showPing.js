class ShowPing {
  constructor() {
    this.originalGenList = window.windows[22].genList;

    window.glorpClient.settings.toggleShowPing = (enabled) =>
      this.toggle(enabled);

    this.toggle(true);
  }

  toggle(enabled) {
    if (enabled) {
      window.windows[22].genList = this.modifiedGenList.bind(this);
    } else window.windows[22].genList = this.originalGenList;
  }

  modifiedGenList() {
    const htmlString = this.originalGenList.call(this);

    const parser = new DOMParser();
    const doc = parser.parseFromString(htmlString, "text/html");
    const pingIcons = doc.querySelectorAll(".pListPing.material-icons");

    for (const icon of pingIcons) {
      const pingValue = icon.getAttribute("title");

      icon.classList.remove("pListPing", "material-icons");
      icon.removeAttribute("title");

      icon.textContent = `${pingValue ? pingValue : "N/A"} `;
    }
    return doc.body.innerHTML;
  }
}

new ShowPing();
