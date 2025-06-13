class RealPing {
    constructor() {
        this.originalPing = 0;
        this.observer = null;
        
        // Wait for glorpClient to be available
        const initRealPing = () => {
            if (window.glorpClient && window.glorpClient.settings) {
                window.glorpClient.settings.toggleRealPing = (enabled) => this.toggle(enabled);
                this.toggle(true);
            } else {
                setTimeout(initRealPing, 100);
            }
        };
        
        initRealPing();
    }

    toggle(enabled) {
        if (enabled) {
            this.startMonitoring();
        } else {
            this.stopMonitoring();
        }
    }

    startMonitoring() {
        // Create a MutationObserver to watch for changes in the ping display
        this.observer = new MutationObserver((mutations) => {
            mutations.forEach((mutation) => {
                if (mutation.type === 'characterData' || mutation.type === 'childList') {
                    this.updatePingDisplay();
                }
            });
        });

        // Start observing the ping text element
        const pingText = document.getElementById('pingText');
        if (pingText) {
            this.observer.observe(pingText, {
                characterData: true,
                childList: true,
                subtree: true
            });
            this.updatePingDisplay();
        }
    }

    stopMonitoring() {
        if (this.observer) {
            this.observer.disconnect();
            this.observer = null;
        }
    }

    updatePingDisplay() {
        const pingText = document.getElementById('pingText');
        if (pingText) {
            const currentPing = parseInt(pingText.textContent);
            if (!isNaN(currentPing)) {
                this.originalPing = currentPing;
                pingText.textContent = (currentPing * 2).toString();
            }
        }
    }
}

// Wait for DOM to be ready before initializing
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => new RealPing());
} else {
    new RealPing();
}