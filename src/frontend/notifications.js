class Notification {
	constructor(message, reqUserInput, duration) {
		this.message = message;
		this.reqUserInput = reqUserInput;
		this.duration = duration;
		if (reqUserInput) {
			this.promise = new Promise((resolve) => {
				this.resolvePromise = resolve;
			});
		}
		this.show();
	}

	async createNotificationElement() {
		this.notificationEl = document.createElement("div");
		this.notificationEl.id = "notification";
		const notificationHtml = await import("./components/notification.html");
		this.notificationEl.innerHTML = notificationHtml.default;

		this.notificationEl.querySelector("#notification-content").textContent = this.message;
		this.notificationEl.querySelector("#notification-timer").textContent = this.duration;
		if (this.reqUserInput) this.notificationEl.querySelector("#notification-actions").style.display = "block";

		document.body.append(this.notificationEl);
	}

	startTimer() {
		this.countdown = setInterval(() => {
			this.duration--;
			this.notificationEl.querySelector("#notification-timer").textContent = this.duration;
			if (this.duration <= 0) {
				clearInterval(this.countdown);
				if (this.resolvePromise) this.resolvePromise(false);
				this.hide();
			}
		}, 1000);
	}

	hide() {
		clearInterval(this.countdown);
		if (this.reqUserInput) document.removeEventListener("keydown", this.handlePromise);
		this.notificationEl.classList.add("slide-out");
		setTimeout(() => this.notificationEl.remove(), 2000);
	}

	show() {
		this.createNotificationElement();
		setTimeout(() => this.notificationEl.classList.add("slide-in"), 10);
		this.startTimer();

		if (this.reqUserInput) {
			this.handlePromise = (event) => {
				if (event.key === "y" || event.key === "n") {
					const action = event.key === "y";

					this.notificationEl.querySelector(`#${event.key}`).classList.add("bounce");

					this.resolvePromise(action);
					this.hide();
				}
			};
			document.addEventListener("keydown", this.handlePromise);
		}
	}
}

window.glorp.showNotification = (message, reqUserInput, seconds) => {
	const notification = new Notification(message, reqUserInput, seconds);
	if (reqUserInput) return notification.promise;

	return notification;
};
