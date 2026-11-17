// Shuusshy website: platform detection, the manual's Windows/macOS switch, the interactive
// hero waveform and pointer-driven motion.
//
// Built to feel like 120 Hz:
// - One requestAnimationFrame loop drives everything that moves with input. It runs at the
//   display's own rate (60, 120, 144 Hz) and uses real elapsed time, so springs and the
//   waveform move the same distance per second on any display, just more smoothly on faster
//   ones. It sleeps when nothing is moving.
// - Input handlers are passive and only record values; the loop applies them in the next frame,
//   so motion never trails the pointer the way CSS transitions do.
// - Only transform/translate/opacity and one small canvas change per frame.
// - Section reveals are CSS scroll-driven animations (compositor). IntersectionObserver picks the
//   active nav item and pauses the waveform when it is off screen.
// No tracking, storage or network requests. Without this script the page is complete and static.
(function () {
  "use strict";
  var doc = document.documentElement;
  var body = document.body;
  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var fine = window.matchMedia("(hover: hover) and (pointer: fine)").matches;
  doc.classList.add("js");

  // ------------------------------------------------------------ spring physics
  // Spring integrated with real elapsed time (semi-implicit Euler, sub-stepped at 240 Hz).
  function Spring(stiffness, damping) {
    this.k = stiffness;
    this.c = damping;
    this.x = 0;
    this.v = 0;
    this.target = 0;
  }
  Spring.prototype.step = function (dt) {
    var n = Math.max(1, Math.ceil(dt / (1 / 240)));
    var h = dt / n;
    for (var i = 0; i < n; i++) {
      var a = -this.k * (this.x - this.target) - this.c * this.v;
      this.v += a * h;
      this.x += this.v * h;
    }
    return Math.abs(this.x - this.target) > 0.001 || Math.abs(this.v) > 0.001;
  };
  Spring.prototype.settle = function () {
    this.x = this.target;
    this.v = 0;
  };

  // ------------------------------------------------------------ the frame loop
  var jobs = []; // functions(dt) -> true while they still need frames
  var running = false;
  var last = 0;
  function loop(now) {
    var dt = Math.min(0.05, (now - last) / 1000 || 1 / 120);
    last = now;
    var busy = false;
    for (var i = 0; i < jobs.length; i++) {
      if (jobs[i](dt)) busy = true;
    }
    if (busy) requestAnimationFrame(loop);
    else running = false;
  }
  function wake() {
    if (!running) {
      running = true;
      last = performance.now();
      requestAnimationFrame(loop);
    }
  }

  // ------------------------------------------------------------ scroll: header + progress
  var header = document.querySelector(".site-header");
  var progress = document.querySelector(".progress");
  var scrollDirty = true;
  jobs.push(function () {
    if (!scrollDirty) return false;
    scrollDirty = false;
    var y = window.scrollY;
    var max = doc.scrollHeight - window.innerHeight;
    if (progress) progress.style.setProperty("--progress", max > 0 ? Math.min(1, y / max).toFixed(4) : "0");
    if (header) header.classList.toggle("scrolled", y > 8);
    return false;
  });
  window.addEventListener(
    "scroll",
    function () {
      scrollDirty = true;
      wake();
    },
    { passive: true },
  );

  // ------------------------------------------------------------ pointer light on glass
  var glow = null;
  jobs.push(function () {
    if (!glow) return false;
    var r = glow.el.getBoundingClientRect();
    glow.el.style.setProperty("--mx", (glow.x - r.left).toFixed(1) + "px");
    glow.el.style.setProperty("--my", (glow.y - r.top).toFixed(1) + "px");
    glow = null;
    return false;
  });
  if (fine) {
    document.addEventListener(
      "pointermove",
      function (e) {
        var g = e.target.closest && e.target.closest(".glass");
        if (g) {
          glow = { el: g, x: e.clientX, y: e.clientY };
          wake();
        }
      },
      { passive: true },
    );
  }

  // ------------------------------------------------------------ 3D tilt toward the pointer
  function tilt(el, area, maxDeg) {
    var rx = new Spring(180, 22);
    var ry = new Spring(180, 22);
    area.addEventListener(
      "pointermove",
      function (e) {
        var r = area.getBoundingClientRect();
        rx.target = -((e.clientY - r.top) / r.height - 0.5) * maxDeg;
        ry.target = ((e.clientX - r.left) / r.width - 0.5) * maxDeg * 1.2;
        wake();
      },
      { passive: true },
    );
    area.addEventListener("pointerleave", function () {
      rx.target = 0;
      ry.target = 0;
      wake();
    });
    jobs.push(function (dt) {
      var a = rx.step(dt);
      var b = ry.step(dt);
      el.style.setProperty("--rx", rx.x.toFixed(3) + "deg");
      el.style.setProperty("--ry", ry.x.toFixed(3) + "deg");
      return a || b;
    });
  }

  // ------------------------------------------------------------ magnetic buttons
  function magnetic(btn) {
    var sx = new Spring(260, 20);
    var sy = new Spring(260, 20);
    btn.addEventListener(
      "pointermove",
      function (e) {
        var r = btn.getBoundingClientRect();
        sx.target = (e.clientX - (r.left + r.width / 2)) * 0.18;
        sy.target = (e.clientY - (r.top + r.height / 2)) * 0.28;
        wake();
      },
      { passive: true },
    );
    btn.addEventListener("pointerleave", function () {
      sx.target = 0;
      sy.target = 0;
      wake();
    });
    jobs.push(function (dt) {
      var a = sx.step(dt);
      var b = sy.step(dt);
      btn.style.setProperty("--tx", sx.x.toFixed(2) + "px");
      btn.style.setProperty("--ty", sy.x.toFixed(2) + "px");
      return a || b;
    });
  }

  if (fine && !reduce) {
    var demoEl = document.querySelector(".demo");
    if (demoEl) tilt(demoEl, demoEl, 7);
    var shot = document.querySelector(".shot-tilt");
    if (shot) tilt(shot.querySelector(".tilt"), shot, 10);
    document.querySelectorAll(".hero .cta-row .btn").forEach(magnetic);
  }

  // ------------------------------------------------------------ hero waveform demo
  (function () {
    var demo = document.querySelector(".demo");
    if (!demo) return;
    var canvas = demo.querySelector(".wave");
    var toggle = demo.querySelector(".toggle");
    var state = demo.querySelector(".demo-state");
    var ctx = canvas.getContext("2d");
    var noise = new Spring(70, 15); // 1 = noisy, 0 = cleaned
    var visible = true;
    var w = 0;
    var h = 0;
    var dpr = 1;
    var t = 1.3;
    var touched = false;

    function size() {
      dpr = Math.min(2, window.devicePixelRatio || 1);
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      draw();
    }

    function set(on, user) {
      if (user) touched = true;
      toggle.setAttribute("aria-checked", String(on));
      demo.setAttribute("data-on", String(on));
      state.textContent = on ? "Clean voice" : "Background noise";
      noise.target = on ? 0 : 1;
      if (reduce) noise.settle();
      draw();
      wake();
    }

    var CYAN = "#39d5ff";
    var AMBER = "#ffbd5c";

    // Smooth pseudo-random noise in [0, 1], continuous in time.
    function hash(i) {
      var s = Math.sin(i * 127.1) * 43758.5453;
      return s - Math.floor(s);
    }
    function vnoise(i, time) {
      var a = Math.floor(time);
      var f = time - a;
      f = f * f * (3 - 2 * f);
      return hash(i * 31.7 + a) * (1 - f) + hash(i * 31.7 + a + 1) * f;
    }

    function draw() {
      if (!w) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      var mid = h / 2;
      var bw = 3;
      var gap = 3;
      var n = Math.floor((w - 16) / (bw + gap));
      var x0 = (w - n * (bw + gap) + gap) / 2;
      var m = Math.max(0, Math.min(1.2, noise.x));
      var noiseAlpha = 0.18 + 0.5 * Math.min(1, m);
      ctx.globalAlpha = 1;
      ctx.fillStyle = "rgba(255,255,255,0.06)";
      ctx.fillRect(8, mid - 0.5, w - 16, 1);
      for (var i = 0; i < n; i++) {
        var u = i / n;
        // Speech: syllable envelopes travelling across, with a fine carrier.
        var env = Math.max(0, Math.sin(Math.PI * 2 * (u * 2.6 - t * 0.45)));
        env = 0.06 + 0.94 * Math.pow(env, 1.25) * (0.6 + 0.4 * Math.sin(Math.PI * 2 * (u * 0.9 - t * 0.13)));
        var voice = env * (0.62 + 0.38 * Math.sin(u * 61 + t * 11));
        // Noise: steady hiss plus occasional clicks (a keyboard).
        var hiss = 0.16 + 0.22 * vnoise(i, t * 9);
        var click = Math.pow(vnoise(i * 3.3, t * 2.2), 14) * 1.8;
        var nz = (hiss + click) * (0.05 + 0.95 * m);
        var vh = Math.max(1.5, voice * (mid - 10));
        var th = Math.max(vh, Math.min(mid - 6, (voice + nz) * (mid - 10)));
        var x = x0 + i * (bw + gap);
        if (th > vh + 0.5) {
          ctx.globalAlpha = noiseAlpha;
          ctx.fillStyle = AMBER;
          ctx.fillRect(x, mid - th, bw, th * 2);
        }
        ctx.globalAlpha = 0.55 + 0.45 * env;
        ctx.fillStyle = CYAN;
        ctx.fillRect(x, mid - vh, bw, vh * 2);
      }
    }

    jobs.push(function (dt) {
      if (!visible || document.hidden || reduce) return false;
      t += dt;
      noise.step(dt);
      draw();
      return true;
    });

    toggle.addEventListener("click", function () {
      set(toggle.getAttribute("aria-checked") !== "true", true);
    });
    if ("ResizeObserver" in window) new ResizeObserver(size).observe(canvas);
    else window.addEventListener("resize", size, { passive: true });
    if ("IntersectionObserver" in window) {
      new IntersectionObserver(function (en) {
        visible = en[0].isIntersecting;
        if (visible) wake();
      }).observe(canvas);
    }
    document.addEventListener("visibilitychange", wake);

    // Start noisy, then switch noise cancellation on once, to show what it does.
    size();
    set(false, false);
    noise.settle();
    draw();
    if (reduce) set(true, false);
    else
      setTimeout(function () {
        if (!touched) set(true, false);
      }, 1700);
  })();

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
    document.querySelectorAll("[data-set-os]").forEach(function (b) {
      b.setAttribute("aria-pressed", String(b.getAttribute("data-set-os") === os));
    });
    placePill();
    if (animate && !reduce) {
      document.querySelectorAll(os === "mac" ? ".topics .os-mac" : ".topics .os-win").forEach(function (el) {
        el.classList.remove("os-swap");
        void el.offsetWidth; // restart the animation
        el.classList.add("os-swap");
      });
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

  // ------------------------------------------------------------ active nav item
  var navLinks = document.querySelectorAll('.nav a[href^="#"]:not(.header-cta)');
  if ("IntersectionObserver" in window && navLinks.length) {
    var byId = {};
    navLinks.forEach(function (a) {
      byId[a.getAttribute("href").slice(1)] = a;
    });
    var navIo = new IntersectionObserver(
      function (entries) {
        // Every section is watched; one without a nav item clears the highlight.
        entries.forEach(function (en) {
          if (!en.isIntersecting) return;
          navLinks.forEach(function (x) {
            x.removeAttribute("aria-current");
          });
          var a = byId[en.target.id];
          if (a) a.setAttribute("aria-current", "true");
        });
      },
      { rootMargin: "-45% 0px -50% 0px" },
    );
    document.querySelectorAll("main > section").forEach(function (s) {
      navIo.observe(s);
    });
  }

  // ------------------------------------------------------------ smooth <details>
  // Web Animations API height animation: short, and a second click mid-way reverses it.
  function animateDetails(d) {
    var summary = d.querySelector("summary");
    var anim = null;
    summary.addEventListener("click", function (e) {
      if (reduce || !d.animate) return;
      e.preventDefault();
      var start = d.offsetHeight;
      if (anim) anim.cancel();
      var opening = !d.open || d.classList.contains("closing");
      d.classList.add("animating");
      d.classList.toggle("closing", !opening);
      if (opening) d.open = true;
      var end = opening ? d.scrollHeight : summary.offsetHeight;
      anim = d.animate(
        { height: [start + "px", end + "px"] },
        { duration: opening ? 380 : 260, easing: "cubic-bezier(0.16, 1, 0.3, 1)" },
      );
      anim.onfinish = function () {
        anim = null;
        if (!opening) d.open = false;
        d.classList.remove("animating", "closing");
      };
    });
  }
  document.querySelectorAll("details.topic, .faq details").forEach(animateDetails);

  // ------------------------------------------------------------ images fade in once decoded
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

  wake();
})();
