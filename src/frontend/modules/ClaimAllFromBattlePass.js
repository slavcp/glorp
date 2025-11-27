(function () {
    function findClaimables() {
        return Array.from(document.querySelectorAll(".bpClaimB")).filter(btn =>
            btn.offsetParent !== null &&
            btn.textContent.trim() === "Claim"
        );
    }

    async function claimEverything() {
        const items = findClaimables();
        if (items.length === 0) return;

        for (const btn of items) {
            const code = btn.getAttribute("onclick");
            if (code && code.includes("windows[5].claimItem(")) {
                try { eval(code); } catch (_) {}
            }
            await new Promise(r => setTimeout(r, 200));
        }

        updateButtonState();
    }

    function updateButtonState() {
        let el = document.getElementById("claimAllBtn");
        if (!el) {
            addClaimAllButton();
            el = document.getElementById("claimAllBtn");
            if (!el) return;
        }

        const hasClaimable = findClaimables().length > 0;
        el.disabled = !hasClaimable;
        el.textContent = hasClaimable ? "Claim All" : "Nothing to Claim!";
    }

    function addClaimAllButton() {
        const bar = document.querySelector(".bpBotH");
        if (!bar) return;

        if (document.getElementById("claimAllBtn")) return;

        const el = document.createElement("div");
        el.className = "bpBtn skip bpClaimAllBtn";
        el.id = "claimAllBtn";
        el.textContent = "Claim All";
        el.onclick = () => {
            window.playSelect?.(0.1);
            claimEverything();
        };

        bar.append(el);
        updateButtonState();
    }

    const obs = new MutationObserver(() => {
        addClaimAllButton();
        updateButtonState();
    });
    obs.observe(document.body, { childList: true, subtree: true });

    const wait = setInterval(() => {
        if (document.querySelector(".bpBotH")) {
            addClaimAllButton();
            clearInterval(wait);
        }
    }, 300);
})();
