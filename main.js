document.addEventListener("DOMContentLoaded", function () {
  const words = [
    "innovative",
    "unique",
    "lightweight",
    "amazing",
    "fantastic",
    "awesome",
    "superb",
    "excellent",
    "performant",
  ];
  let wordIndex = 0;
  let charIndex = 0;
  let isDeleting = false;
  let pauseTimeout;
  const typewriter = document.getElementById("typewriter");

  function type() {
    const currentWord = words[wordIndex];
    if (isDeleting) {
      charIndex--;
      typewriter.innerHTML =
        currentWord.substring(0, charIndex) +
        '<span class="typewriter-cursor">_</span>';
      if (charIndex === 0) {
        isDeleting = false;
        wordIndex = (wordIndex + 1) % words.length;
        pauseTimeout = setTimeout(type, 500);
        return;
      }
    } else {
      charIndex++;
      typewriter.innerHTML =
        currentWord.substring(0, charIndex) +
        '<span class="typewriter-cursor">_</span>';
      if (charIndex === currentWord.length) {
        isDeleting = true;
        pauseTimeout = setTimeout(type, 1000);
        return;
      }
    }
    pauseTimeout = setTimeout(type, isDeleting ? 40 : 70);
  }
  type();

  const downloadBtn = document.getElementById("download-btn");
  fetch("https://api.github.com/repos/slavcp/glorp/releases/latest")
    .then((response) => {
      if (!response.ok) throw new Error("Failed to fetch release");
      return response.json();
    })
    .then((data) => {
      if (data.assets && data.assets.length > 0) {
        downloadBtn.href = data.assets[0].browser_download_url;
        downloadBtn.textContent = "Download";
        downloadBtn.title = "Download";
        downloadBtn.classList.remove("disabled");
        downloadBtn.removeAttribute("aria-disabled");
      } else {
        downloadBtn.textContent = "Error";
        downloadBtn.title = "Error download";
        downloadBtn.classList.add("disabled");
        downloadBtn.setAttribute("aria-disabled", "true");
      }
    })
    .catch(() => {
      downloadBtn.textContent = "Error";
      downloadBtn.title = "Error download";
      downloadBtn.classList.add("disabled");
      downloadBtn.setAttribute("aria-disabled", "true");
    });
  downloadBtn.addEventListener("click", function (e) {
    if (
      downloadBtn.classList.contains("disabled") ||
      downloadBtn.getAttribute("aria-disabled") === "true"
    ) {
      e.preventDefault();
    }
  });
});
