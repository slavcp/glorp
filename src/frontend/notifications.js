class Notification {
  constructor(message, reqUserInput, duration) {
    this.message = message;
    this.reqUserInput = reqUserInput;
    this.duration = duration; // in seconds
    this.notificationEl = null;
    if (reqUserInput) {
      this.promise = new Promise((resolve) => {
        this.resolvePromise = resolve;
        this.show();
      });
    }
    this.show();
  }

  createNotificationElement() {
    this.notificationEl = document.createElement("div");
    this.notificationEl.id = "notification";
    this.notificationEl.innerHTML = `
            <style>
                #notification {
                    position: absolute;
                    display: flex;
                    top: 50px;
                    right: -350px;
                    background-color: #333;
                    padding: 15px 20px;
                    border-radius: 8px;
                    box-shadow: 0px 0px 8px 2px black;
                    font-family: gamefont;
                    font-size: 14px;
                    width: 300px;
                    height: 50px;
                    z-index: 999;
                }

                #notification.slide-in {
                    transform: translateX(-380px);
                    transition: all 1s cubic-bezier(0.87, 0, 0.13, 1);
                }

                #notification.slide-out {
                    transform: translateX(380px);
                    transition: all 1s cubic-bezier(0.87, 0, 0.13, 1);
                }

                #notification-content {
                    text-align: center;
                    align-self: center;
                    color: white;
                    width: 100%;
                    flex-wrap: nowrap;
                }

                #notification-timer {
                    position: absolute;
                    bottom: 0;
                    right: 0;
                    margin-right: 5px;
                    margin-bottom: 2px;
                    justify-content: flex-end;
                    font-size: 12px;
                    color: white;
                }
                #notification-actions {
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    position: absolute;
                    left: 0;
                    bottom: 0;
                    margin-left: 20px;
                    margin-bottom: 5px;
                    font-size:10px;
                }

                .bounce {
                    transform: scale(1.2)!important;
                    transition: transform 0.2s ease-in-out!important;
                    display: inline-block!important;
                }
            </style>
            <div id="notification-content">${this.message}</div>
            <div id="notification-timer">${this.duration}</div>
            <div id="notification-actions" style="${
              this.type === 1 ? "" : "display: none;"
            }">
                <span id="y" style="background-color: #444; border-radius: 4px; padding: 2px 4px; color: white; margin-right: 3px; display: inline-block;">Y</span>
                <span id="n" style="background-color: #444; border-radius: 4px; padding: 2px 4px; color: white; margin-right: 3px; display: inline-block;">N</span>
            </div>
        `;
    document.body.appendChild(this.notificationEl);
  }

  startTimer() {
    const countdown = setInterval(() => {
      this.duration--;
      this.notificationEl.querySelector("#notification-timer").textContent =
        this.duration;
      if (this.duration <= 0) {
        clearInterval(countdown);
        setTimeout(() => this.notificationEl.remove(), 2000);
      }
    }, 1000);
  }

  show() {
    this.createNotificationElement();
    setTimeout(() => this.notificationEl.classList.add("slide-in"), 10);
    this.startTimer();

    if (this.reqUserInput) {
      const handleKeyDown = (event) => {
        if (event.key === "y" || event.key === "n") {
          const action = event.key === "y";

          this.notificationEl.classList.add("slide-out");
          this.notificationEl
            .querySelector(`#${event.key}`)
            .classList.add("bounce");
          document.removeEventListener("keydown", handleKeyDown);

          if (this.resolvePromise) this.resolvePromise(action);

          setTimeout(() => this.notificationEl.remove(), 2000);
        }
      };
      document.addEventListener("keydown", handleKeyDown);

      // auto-resolve with false if no response
      setTimeout(() => {
        this.notificationEl.classList.add("slide-out");
        document.removeEventListener("keydown", handleKeyDown);
        if (this.resolvePromise) this.resolvePromise(false);
        setTimeout(() => this.notificationEl.remove(), 2000);
      }, this.duration * 1000);
    } else {
      setTimeout(() => {
        this.notificationEl.classList.add("slide-out");
        setTimeout(() => this.notificationEl.remove(), 2000);
      }, this.duration * 1000);
    }
  }
}

window.glorpClient.showNotification = (message, type, seconds) => {
  return new Notification(message, type, seconds);
};
