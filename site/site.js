// Shuusshy website: platform detection, the manual's Windows/macOS switch, and fluid motion.
//
// Performance rules (so motion keeps up with 60/120/144 Hz displays):
// - Only transform and opacity are animated; nothing that triggers layout while scrolling.
// - Scroll and pointer handlers are passive and only record values; one requestAnimationFrame
//   per frame applies them.
// - Reveals are CSS scroll-driven animations (compositor); IntersectionObserver picks the
//   active nav item.
// No tracking, no storage, no network requests. Without this script the page is complete and
// static (Windows first).
(function () {
  "use strict";
  var doc = document.documentElement;
  var body = document.body;
  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
  var finePointer = window.matchMedia("(hover: hover) and (pointer: fine)");
  doc.classList.add("js");

  // ------------------------------------------------------------ platform + switch
  var isMac = /Macintosh|Mac OS X/.test(navigator.userAgent) && !/iPhone|iPad/.test(navigator.userAgent);
  var sw = document.querySelector(".switch");
  var pill = sw && sw.querySelector(".pill");

  function placePill() {
    if (!pill) return;
    var on = sw.querySelector('[aria-pressed="true"]');
    if (!on) return;
    pill.style.setProperty("--px", on.offsetLeft + "px");
    pill.style.setProperty("--pw", on.offsetWidth + "px");
  }

  function setOs(os, animate) {
    if (body.getAttribute("data-os") === os) return;
    body.setAttribute("data-os", os);
    var buttons = document.querySelectorAll("[data-set-os]");
    for (var i = 0; i < buttons.length; i++) {
      buttons[i].setAttribute("aria-pressed", String(buttons[i].getAttribute("data-set-os") === os));
    }
    placePill();
    if (animate && !reduce.matches) {
      var shown = document.querySelectorAll(os === "mac" ? ".topics .os-mac" : ".topics .os-win");
      for (var j = 0; j < shown.length; j++) {
        var el = shown[j];
        el.classList.remove("os-swap");
        void el.offsetWidth; // restart the animation
        el.classList.add("os-swap");
      }
    }
  }

  if (isMac) setOs("mac", false);
  if (sw) {
    doc.classList.add("js-switch");
    placePill();
    window.addEventListener("resize", placePill, { passive: true });
  }
  document.addEventListener("click", function (e) {
    var b = e.target.closest && e.target.closest("[data-set-os]");
    if (b) setOs(b.getAttribute("data-set-os"), true);
  });

  // ------------------------------------------------------------ frame loop
  var header = document.querySelector(".site-header");
  var progress = document.querySelector(".progress");
  var tilt = document.querySelector(".hero-shot .tilt");
  var pending = false;
  var scrollY = window.scrollY;
  var pointer = null; // { x, y, target }
  var tiltTarget; // { rx, ry }, null to reset, undefined = no change

  function frame() {
    pending = false;
    var max = doc.scrollHeight - window.innerHeight;
    if (progress) progress.style.setProperty("--progress", max > 0 ? Math.min(1, scrollY / max).toFixed(4) : "0");
    if (header) header.classList.toggle("scrolled", scrollY > 8);
    if (pointer) {
      var g = pointer.target;
      var r = g.getBoundingClientRect();
      g.style.setProperty("--mx", (pointer.x - r.left).toFixed(0) + "px");
      g.style.setProperty("--my", (pointer.y - r.top).toFixed(0) + "px");
      pointer = null;
    }
    if (tilt && tiltTarget !== undefined) {
      if (tiltTarget) {
        tilt.classList.add("tracking");
        tilt.style.setProperty("--rx", tiltTarget.rx.toFixed(2) + "deg");
        tilt.style.setProperty("--ry", tiltTarget.ry.toFixed(2) + "deg");
      } else {
        tilt.classList.remove("tracking");
        tilt.style.setProperty("--rx", "0deg");
        tilt.style.setProperty("--ry", "0deg");
      }
      tiltTarget = undefined;
    }
  }

  function schedule() {
    if (!pending) {
      pending = true;
      requestAnimationFrame(frame);
    }
  }

  window.addEventListener(
    "scroll",
    function () {
      scrollY = window.scrollY;
      schedule();
    },
    { passive: true },
  );
  schedule();

  // Pointer light on glass panels, and the hero tilt (mouse / trackpad only).
  if (finePointer.matches) {
    document.addEventListener(
      "pointermove",
      function (e) {
        var g = e.target.closest && e.target.closest(".glass");
        if (g) pointer = { x: e.clientX, y: e.clientY, target: g };
        schedule();
      },
      { passive: true },
    );
    var hero = document.querySelector(".hero-shot");
    if (hero && tilt && !reduce.matches) {
      hero.addEventListener(
        "pointermove",
        function (e) {
          var r = hero.getBoundingClientRect();
          var px = (e.clientX - r.left) / r.width - 0.5;
          var py = (e.clientY - r.top) / r.height - 0.5;
          tiltTarget = { rx: -py * 9, ry: px * 11 };
          schedule();
        },
        { passive: true },
      );
      hero.addEventListener("pointerleave", function () {
        tiltTarget = null;
        schedule();
      });
    }
  }

  // ------------------------------------------------------------ active nav item
  var navLinks = document.querySelectorAll('.nav a[href^="#"]:not(.header-cta)');
  if ("IntersectionObserver" in window && navLinks.length) {
    var byId = {};
    navLinks.forEach(function (a) {
      byId[a.getAttribute("href").slice(1)] = a;
    });
    var navIo = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (en) {
          var a = byId[en.target.id];
          if (!a) return;
          if (en.isIntersecting) {
            navLinks.forEach(function (x) {
              x.removeAttribute("aria-current");
            });
            a.setAttribute("aria-current", "true");
          }
        });
      },
      { rootMargin: "-45% 0px -50% 0px" },
    );
    Object.keys(byId).forEach(function (id) {
      var s = document.getElementById(id);
      if (s) navIo.observe(s);
    });
  }

  // ------------------------------------------------------------ smooth <details>
  // Animates the height with the Web Animations API: short, interruptible, and only this one
  // element changes size (the rest of the page moves, it does not re-render).
  function animateDetails(d) {
    var summary = d.querySelector("summary");
    var anim = null;
    summary.addEventListener("click", function (e) {
      if (reduce.matches || !d.animate) return;
      e.preventDefault();
      if (anim) anim.cancel();
      var start = d.offsetHeight;
      var opening = !d.open;
      d.classList.add("animating");
      if (opening) d.open = true;
      var end = opening ? d.scrollHeight : summary.offsetHeight;
      if (!opening) {
        // Measure the closed height precisely.
        end = summary.offsetHeight + (d.offsetHeight - d.clientHeight);
      }
      anim = d.animate(
        { height: [start + "px", end + "px"] },
        { duration: opening ? 420 : 300, easing: "cubic-bezier(0.16, 1, 0.3, 1)" },
      );
      anim.onfinish = function () {
        anim = null;
        if (!opening) d.open = false;
        d.classList.remove("animating");
      };
      anim.oncancel = function () {
        d.classList.remove("animating");
      };
    });
  }
  document.querySelectorAll("details.topic, .faq details").forEach(animateDetails);

  // ------------------------------------------------------------ images fade in
  doc.classList.add("js-img");
  document.querySelectorAll('img[loading="lazy"]').forEach(function (img) {
    function done() {
      img.classList.add("loaded");
    }
    if (img.complete && img.naturalWidth) done();
    else {
      img.addEventListener("load", done, { once: true });
      img.addEventListener("error", done, { once: true });
    }
  });
})();
