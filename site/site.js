// Shuusshy website: show the visitor's platform first and switch the manual between
// Windows and macOS. No tracking, no storage, no network requests. Without this script the
// page shows the Windows version and all links still work.
(function () {
  var body = document.body;
  var isMac = /Macintosh|Mac OS X/.test(navigator.userAgent) && !/iPhone|iPad/.test(navigator.userAgent);

  function setOs(os) {
    body.setAttribute("data-os", os);
    var buttons = document.querySelectorAll("[data-set-os]");
    for (var i = 0; i < buttons.length; i++) {
      buttons[i].setAttribute("aria-pressed", String(buttons[i].getAttribute("data-set-os") === os));
    }
  }

  if (isMac) setOs("mac");

  document.addEventListener("click", function (e) {
    var b = e.target.closest && e.target.closest("[data-set-os]");
    if (b) setOs(b.getAttribute("data-set-os"));
  });
})();
