const affinityPanel = document.createElement('div');
affinityPanel.id = 'affinityPanel';
affinityPanel.style.display = 'none';
affinityPanel.innerHTML = `
  <div class="popup" style="width: 400px;">
    <div class="popupHeader">
      <span>CPU Affinity Settings</span>
      <div class="closeBtn" onclick="window.glorpClient.settings.closeAffinityPanel()">×</div>
    </div>
    <div class="popupContent" style="padding: 20px;">
      <div style="margin-bottom: 15px;">
        <div style="font-weight: bold; margin-bottom: 8px;">GPU Process</div>
        <div id="gpuCores" class="coreSelector"></div>
      </div>
      <div>
        <div style="font-weight: bold; margin-bottom: 8px;">Webpage Process</div>
        <div id="webpageCores" class="coreSelector"></div>
      </div>
      <div class="settingsBtn" style="margin-top: 15px;" onclick="window.glorpClient.settings.applyAffinity()">Apply</div>
    </div>
  </div>
`;

document.body.appendChild(affinityPanel);

const style = document.createElement('style');
style.textContent = `
  #affinityPanel {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.5);
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .coreSelector {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .core-checkbox {
    display: inline-flex;
    align-items: center;
    padding: 5px 10px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 4px;
    cursor: pointer;
    user-select: none;
  }
  .core-checkbox:hover {
    background: rgba(0, 0, 0, 0.3);
  }
  .core-checkbox input {
    margin-right: 5px;
  }
`;

document.head.appendChild(style);

function initCoreSelectors() {
  const cpuCount = navigator.hardwareConcurrency || 4;
  const gpuCores = document.getElementById('gpuCores');
  const webpageCores = document.getElementById('webpageCores');
  
  [gpuCores, webpageCores].forEach((container, processIndex) => {
    container.innerHTML = '';
    for (let i = 0; i < cpuCount; i++) {
      const label = document.createElement('label');
      label.className = 'core-checkbox';
      label.innerHTML = `
        <input type="checkbox" value="${i}" data-process="${processIndex}">
        Core ${i}
      `;
      container.appendChild(label);
    }
  });
}
window.glorpClient.settings.openAffinityPanel = () => {
  initCoreSelectors();
  affinityPanel.style.display = 'flex';
};

window.glorpClient.settings.closeAffinityPanel = () => {
  affinityPanel.style.display = 'none';
};

window.glorpClient.settings.applyAffinity = () => {
  const gpuMask = Array.from(document.querySelectorAll('#gpuCores input:checked'))
    .reduce((mask, input) => mask | (1 << parseInt(input.value)), 0);
  
  const webpageMask = Array.from(document.querySelectorAll('#webpageCores input:checked'))
    .reduce((mask, input) => mask | (1 << parseInt(input.value)), 0);

  window.chrome.webview.postMessage(`setAffinity,${gpuMask},${webpageMask}`);
  window.glorpClient.settings.closeAffinityPanel();
};
window.glorpClient.settings.toggleCpuAffinity = (enabled) => {
  if (!enabled) {
    window.chrome.webview.postMessage('resetAffinity');
  }
};