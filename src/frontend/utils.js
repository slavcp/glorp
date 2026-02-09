window.hook = (target, method, wrapper) => {
	const original = target.prototype[method];
	target.prototype[method] = function (...args) {
		const result = wrapper.call(this, args, original);
		return result === undefined ? original.apply(this, args) : result;
	};
};

window.waitForElement = (selector) => {
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
