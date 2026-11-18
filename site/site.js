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
  var sheen = null;
  jobs.push(function () {
    if (!sheen) return false;
    var r = sheen.el.getBoundingClientRect();
    sheen.el.style.setProperty("--lx", (sheen.x - r.left).toFixed(1) + "px");
    sheen.el.style.setProperty("--ly", (sheen.y - r.top).toFixed(1) + "px");
    sheen = null;
    return false;
  });
  // Touch: the sheen starts where the finger lands.
  document.addEventListener(
    "pointerdown",
    function (e) {
      var b = e.target.closest && e.target.closest(".btn, .header-cta");
      if (b) {
        sheen = { el: b, x: e.clientX, y: e.clientY };
        wake();
      }
    },
    { passive: true },
  );
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
        var b = e.target.closest && e.target.closest(".btn, .header-cta");
        if (b) {
          sheen = { el: b, x: e.clientX, y: e.clientY };
          wake();
        }
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

  // ------------------------------------------------------------ liquid glass lens
  // Moves a selection marker on springs. While it travels it stretches along its path and
  // thins across it, like a drop of liquid; the slightly underdamped spring lets it overshoot
  // and wobble once as it settles. Returns move(position, size).
  function liquidLens(el, axis) {
    var pos = new Spring(320, 23);
    var size = new Spring(320, 26);
    var placed = false;
    el.style.transition = "none";
    jobs.push(function (dt) {
      var a = pos.step(dt);
      var b = size.step(dt);
      var st = reduce ? 0 : Math.min(0.32, Math.abs(pos.v) / 2600);
      var sx = axis === "x" ? 1 + st : 1 - st * 0.45;
      var sy = axis === "x" ? 1 - st * 0.45 : 1 + st;
      el.style[axis === "x" ? "width" : "height"] = size.x.toFixed(1) + "px";
      el.style.transform =
        (axis === "x" ? "translate3d(" + pos.x.toFixed(2) + "px,0,0)" : "translate3d(0," + pos.x.toFixed(2) + "px,0)") +
        " scale(" + sx.toFixed(4) + "," + sy.toFixed(4) + ")";
      return a || b;
    });
    return function (p, s) {
      pos.target = p;
      size.target = s;
      if (!placed || reduce) {
        pos.settle();
        size.settle();
        placed = true;
      }
      wake();
    };
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


  // ------------------------------------------------------------ the grey gradients
  // Every section is a window onto its own fixed gradient of two greys (pale for light
  // sections, dark for dark ones), picked at random for this visit. Two glows float on their
  // own paths as the page scrolls; springs smooth the scroll so they glide at the display's own
  // rate. The header takes the tone of the section under it. A warm glow fades in over the hero
  // while its noise cancellation is off. Transform and opacity only; the loop sleeps when idle.
  var setFieldNoise = function () {};
  (function () {
    var bands = document.querySelectorAll(".band[data-shade]");
    var hdr = document.querySelector(".site-header");
    if (!bands.length) return;

    // Two greys per tone, a hair warm or cool. Ranges keep secondary text at 4.5:1 or more:
    // pale greys 208-246, dark greys 14-50.
    function grey(lo, hi) {
      var v = lo + Math.floor(Math.random() * (hi - lo + 1));
      var tint = Math.floor(Math.random() * 5) - 2;
      return "rgb(" + (v + tint) + " " + v + " " + (v - tint + 2) + ")";
    }
    doc.style.setProperty("--l1", grey(234, 246));
    doc.style.setProperty("--l2", grey(208, 220));
    doc.style.setProperty("--d1", grey(38, 50));
    doc.style.setProperty("--d2", grey(14, 22));

    var windows = [];
    bands.forEach(function (band) {
      var bg = document.createElement("div");
      bg.className = "bg";
      bg.setAttribute("aria-hidden", "true");
      var a = document.createElement("i");
      a.className = "blob ba";
      var b = document.createElement("i");
      b.className = "blob bb";
      bg.append(a, b);
      band.insertBefore(bg, band.firstChild);
      windows.push({ band: band, bg: bg, a: a, b: b, on: true });
    });
    doc.classList.add("js-bg");
    var warm = document.querySelector(".field-noise");
    var heroBg = document.querySelector(".hero > .bg");
    if (warm && heroBg) heroBg.appendChild(warm);

    // Only windows on screen are drawn and moved.
    if ("IntersectionObserver" in window) {
      var vis = new IntersectionObserver(function (entries) {
        entries.forEach(function (en) {
          for (var k = 0; k < windows.length; k++) {
            if (windows[k].band === en.target) {
              windows[k].on = en.isIntersecting;
              windows[k].bg.style.visibility = en.isIntersecting ? "visible" : "hidden";
            }
          }
        });
        wake();
      });
      windows.forEach(function (w) {
        vis.observe(w.band);
      });
      // The header takes the tone of the section under it.
      var top = new IntersectionObserver(
        function (entries) {
          entries.forEach(function (en) {
            if (!en.isIntersecting || !hdr) return;
            var dark = en.target.classList.contains("tone-dark");
            hdr.classList.toggle("tone-dark", dark);
            hdr.classList.toggle("tone-light", !dark);
          });
        },
        { rootMargin: "0px 0px -94% 0px" },
      );
      windows.forEach(function (w) {
        top.observe(w.band);
      });
    }

    var px = new Spring(26, 9);
    var py = new Spring(26, 9);
    // Scroll position in screen heights, eased: the glows glide rather than jump.
    var sc = new Spring(40, 12.5);
    var nz = new Spring(40, 12);
    var phase = Math.random() * 6.28;
    var lastNoise = -1;
    if (fine && !reduce) {
      window.addEventListener(
        "pointermove",
        function (e) {
          px.target = e.clientX / window.innerWidth - 0.5;
          py.target = e.clientY / window.innerHeight - 0.5;
          wake();
        },
        { passive: true },
      );
    }
    function readScroll() {
      sc.target = window.scrollY / Math.max(1, window.innerHeight);
      if (reduce) sc.settle();
    }
    readScroll();
    sc.settle();
    window.addEventListener(
      "scroll",
      function () {
        readScroll();
        wake();
      },
      { passive: true },
    );
    setFieldNoise = function (v) {
      var t = Math.max(0, Math.min(1, v));
      if (t === nz.target) return;
      nz.target = t;
      if (reduce) nz.settle();
      wake();
    };
    function tf(x, y, s) {
      return "translate3d(" + x.toFixed(1) + "px," + y.toFixed(1) + "px,0) scale(" + s.toFixed(4) + ")";
    }
    jobs.push(function (dt) {
      var moving = px.step(dt) | py.step(dt) | sc.step(dt) | nz.step(dt);
      var w = window.innerWidth;
      var h = window.innerHeight;
      // The two glows circle on different paths and speeds as the page scrolls, and lean
      // gently toward the pointer.
      var t = sc.x + phase;
      var ox = px.x * 40;
      var oy = py.x * 30;
      var ta = tf(Math.sin(t * 0.85) * 0.24 * w + ox, Math.cos(t * 0.62) * 0.2 * h + oy, 1 + 0.08 * Math.sin(t * 0.5));
      var tb = tf(Math.cos(t * 0.7 + 2) * 0.26 * w - ox * 0.6, Math.sin(t * 0.95 + 1) * 0.22 * h - oy * 0.6, 1 + 0.08 * Math.cos(t * 0.45));
      for (var k = 0; k < windows.length; k++) {
        if (!windows[k].on) continue;
        windows[k].a.style.transform = ta;
        windows[k].b.style.transform = tb;
      }
      if (warm) {
        var o = Math.max(0, Math.min(1, nz.x));
        if (Math.abs(o - lastNoise) > 0.002) {
          warm.style.opacity = o.toFixed(3);
          warm.style.visibility = o < 0.005 ? "hidden" : "visible";
          lastNoise = o;
        }
      }
      return !!moving;
    });
  })();

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
      setFieldNoise(noise.target);
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
  var movePill = pill ? liquidLens(pill, "x") : null;
  function placePill() {
    if (!pill) return;
    var on = sw.querySelector('[aria-pressed="true"]');
    if (!on) return;
    movePill(on.offsetLeft, on.offsetWidth);
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


  // ------------------------------------------------------------ signal path
  (function () {
    var signal = document.querySelector(".signal");
    if (!signal) return;
    var steps = signal.querySelectorAll(".flow-step");
    var panel = signal.querySelector(".flow-panel");
    function show(btn) {
      steps.forEach(function (b) {
        b.setAttribute("aria-pressed", String(b === btn));
      });
      var detail = btn.parentNode.querySelector(".flow-detail");
      if (panel && detail) {
        panel.replaceChildren();
        detail.childNodes.forEach(function (n) {
          panel.appendChild(n.cloneNode(true));
        });
      }
    }
    steps.forEach(function (b) {
      b.addEventListener("click", function () {
        show(b);
      });
      if (fine) {
        b.addEventListener("pointerenter", function () {
          show(b);
        });
      }
    });
    var start = signal.querySelector('.flow-step[aria-pressed="true"]') || steps[0];
    if (start) show(start);
    // The pulse runs only while the path is on screen.
    if ("IntersectionObserver" in window) {
      new IntersectionObserver(function (en) {
        signal.style.setProperty("--play", en[0].isIntersecting ? "running" : "paused");
      }).observe(signal);
    } else signal.style.setProperty("--play", "running");
  })();

  // ------------------------------------------------------------ features: tabs and stage
  (function () {
    var list = document.querySelector(".sc-tabs");
    if (!list) return;
    var tabs = Array.prototype.slice.call(list.querySelectorAll(".sc-tab"));
    var ink = list.querySelector(".sc-ink");
    var stage = document.querySelector(".sc-stage");
    var imgs = stage.querySelectorAll("img[data-shot]");
    var moveInk = ink ? liquidLens(ink, "y") : null;
    function placeInk() {
      var on = list.querySelector('[aria-selected="true"]');
      if (!ink || !on) return;
      moveInk(on.offsetTop, on.offsetHeight);
    }
    function select(tab, focus) {
      tabs.forEach(function (t) {
        var on = t === tab;
        t.setAttribute("aria-selected", String(on));
        t.tabIndex = on ? 0 : -1;
      });
      stage.setAttribute("aria-labelledby", tab.id);
      var shot = tab.getAttribute("data-shot");
      imgs.forEach(function (img) {
        var on = img.getAttribute("data-shot") === shot;
        img.classList.toggle("on", on);
        if (on) img.removeAttribute("aria-hidden");
        else img.setAttribute("aria-hidden", "true");
      });
      placeInk();
      if (focus) tab.focus({ preventScroll: true });
    }
    tabs.forEach(function (t, i) {
      t.addEventListener("click", function () {
        select(t, false);
      });
      t.addEventListener("keydown", function (e) {
        var k = e.key;
        var j = i;
        if (k === "ArrowDown" || k === "ArrowRight") j = (i + 1) % tabs.length;
        else if (k === "ArrowUp" || k === "ArrowLeft") j = (i - 1 + tabs.length) % tabs.length;
        else if (k === "Home") j = 0;
        else if (k === "End") j = tabs.length - 1;
        else return;
        e.preventDefault();
        select(tabs[j], true);
      });
    });
    select(list.querySelector('[aria-selected="true"]') || tabs[0], false);
    if ("ResizeObserver" in window) new ResizeObserver(placeInk).observe(list);
    else window.addEventListener("resize", placeInk, { passive: true });
    if (fine && !reduce) tilt(stage.querySelector(".tilt"), stage, 6);
  })();

  // ------------------------------------------------------------ user guide index
  // Built from the topics, so the two never disagree. A link opens its topic and scrolls to it;
  // the marker follows the topic in view.
  (function () {
    var nav = document.querySelector(".guide-index");
    var topics = document.querySelectorAll(".guide .topics > details.topic");
    if (!nav || !topics.length) return;
    var ol = nav.querySelector("ol");
    var ink = nav.querySelector(".gi-ink");
    var links = [];
    topics.forEach(function (d, i) {
      if (!d.id) d.id = "guide-" + (i + 1);
      var title = d.querySelector(".topic-title strong");
      var li = document.createElement("li");
      var a = document.createElement("a");
      a.href = "#" + d.id;
      title.childNodes.forEach(function (n) {
        a.appendChild(n.cloneNode(true));
      });
      a.addEventListener("click", function (e) {
        e.preventDefault();
        d.open = true;
        d.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" });
        d.querySelector("summary").focus({ preventScroll: true });
        if (history.replaceState) history.replaceState(null, "", "#" + d.id);
      });
      li.appendChild(a);
      ol.appendChild(li);
      links.push(a);
    });
    nav.hidden = false;
    var current = -1;
    function mark(i) {
      if (i === current) return;
      current = i;
      links.forEach(function (a, j) {
        if (j === i) a.setAttribute("aria-current", "true");
        else a.removeAttribute("aria-current");
      });
      var a = links[i];
      if (ink && a) {
        ink.style.setProperty("--gy", a.offsetTop + "px");
        ink.style.setProperty("--gh", a.offsetHeight + "px");
        ink.style.setProperty("--go", "1");
      }
    }
    if ("IntersectionObserver" in window) {
      var io = new IntersectionObserver(
        function (entries) {
          entries.forEach(function (en) {
            if (en.isIntersecting) mark(Array.prototype.indexOf.call(topics, en.target));
          });
        },
        { rootMargin: "-25% 0px -65% 0px" },
      );
      topics.forEach(function (d) {
        io.observe(d);
      });
    }
    window.addEventListener(
      "resize",
      function () {
        var i = current;
        current = -1;
        if (i >= 0) mark(i);
      },
      { passive: true },
    );
  })();

  // ------------------------------------------------------------ FAQ magazine
  // Contents on the left, one open page on the right, topic filters above. Built from the
  // questions in .faq, which stay the content and the phone layout.
  (function () {
    var mag = document.querySelector(".mag");
    var faq = document.querySelector(".faq");
    if (!mag || !faq) return;
    var items = Array.prototype.slice.call(faq.querySelectorAll("details"));
    var list = mag.querySelector(".mag-list");
    var body = mag.querySelector(".mag-body");
    var page = mag.querySelector(".mag-page");
    var next = mag.querySelector(".mag-next");
    var filters = mag.querySelectorAll(".mag-filters button");
    var names = { privacy: "Privacy", setup: "Setup", fixes: "Fixes", versions: "Versions and Pro" };
    var tabs = [];
    var current = -1;

    function textOf(summary) {
      var t = "";
      summary.childNodes.forEach(function (n) {
        if (n.nodeType === 3) t += n.textContent;
      });
      return t.trim();
    }

    items.forEach(function (d, i) {
      var b = document.createElement("button");
      b.type = "button";
      b.className = "mag-q";
      b.id = "mag-q-" + i;
      b.setAttribute("role", "tab");
      b.setAttribute("aria-controls", "mag-page");
      b.setAttribute("aria-selected", "false");
      b.tabIndex = -1;
      var q = document.createElement("span");
      q.className = "q";
      q.textContent = textOf(d.querySelector("summary"));
      var k = document.createElement("span");
      k.className = "k";
      k.textContent = names[d.getAttribute("data-cat")] || "";
      var dot = document.createElement("span");
      dot.className = "dot";
      dot.setAttribute("aria-hidden", "true");
      b.append(q, dot, k);
      b.addEventListener("click", function () {
        open(i, false);
      });
      b.addEventListener("keydown", function (e) {
        var vis = visible();
        var at = vis.indexOf(i);
        var to = -1;
        if (e.key === "ArrowDown" || e.key === "ArrowRight") to = vis[(at + 1) % vis.length];
        else if (e.key === "ArrowUp" || e.key === "ArrowLeft") to = vis[(at - 1 + vis.length) % vis.length];
        else if (e.key === "Home") to = vis[0];
        else if (e.key === "End") to = vis[vis.length - 1];
        if (to < 0) return;
        e.preventDefault();
        open(to, true);
      });
      list.appendChild(b);
      tabs.push(b);
    });

    function visible() {
      var v = [];
      tabs.forEach(function (t, i) {
        if (!t.hidden) v.push(i);
      });
      return v;
    }

    function open(i, focus) {
      if (i === current) return;
      current = i;
      tabs.forEach(function (t, j) {
        var on = j === i;
        t.setAttribute("aria-selected", String(on));
        t.tabIndex = on ? 0 : -1;
      });
      page.setAttribute("aria-labelledby", tabs[i].id);
      var d = items[i];
      var kicker = document.createElement("span");
      kicker.className = "mag-kicker";
      kicker.textContent = names[d.getAttribute("data-cat")] || "";
      var title = document.createElement("h3");
      title.className = "mag-title";
      title.textContent = textOf(d.querySelector("summary"));
      var answer = document.createElement("div");
      answer.className = "mag-answer";
      d.querySelector(".answer").childNodes.forEach(function (n) {
        answer.appendChild(n.cloneNode(true));
      });
      body.replaceChildren(kicker, title, answer);
      // Restart the page-turn animation.
      body.classList.toggle("swap");
      if (focus) tabs[i].focus({ preventScroll: true });
    }

    next.addEventListener("click", function () {
      var vis = visible();
      var at = vis.indexOf(current);
      open(vis[(at + 1) % vis.length], false);
    });

    var lens = mag.querySelector(".mag-lens");
    var moveLens = lens ? liquidLens(lens, "x") : null;
    function placeLens() {
      var on = mag.querySelector('.mag-filters button[aria-pressed="true"]');
      if (!lens || !on) return;
      lens.style.top = on.offsetTop + "px";
      lens.style.height = on.offsetHeight + "px";
      moveLens(on.offsetLeft, on.offsetWidth);
    }
    filters.forEach(function (f) {
      f.addEventListener("click", function () {
        var cat = f.getAttribute("data-cat");
        filters.forEach(function (x) {
          x.setAttribute("aria-pressed", String(x === f));
        });
        placeLens();
        tabs.forEach(function (t, i) {
          t.hidden = cat !== "all" && items[i].getAttribute("data-cat") !== cat;
        });
        var vis = visible();
        if (vis.indexOf(current) < 0) open(vis[0], false);
        next.hidden = vis.length < 2;
      });
    });

    mag.hidden = false;
    faq.classList.add("has-mag");
    open(0, false);
    // Filtering never changes the spread's height, so the page doesn't jump.
    function holdHeight() {
      list.style.minHeight = "";
      var hidden = tabs.filter(function (t) {
        return t.hidden;
      });
      hidden.forEach(function (t) {
        t.hidden = false;
      });
      list.style.minHeight = list.offsetHeight + "px";
      hidden.forEach(function (t) {
        t.hidden = true;
      });
    }
    holdHeight();
    placeLens();
    window.addEventListener(
      "resize",
      function () {
        holdHeight();
        placeLens();
      },
      { passive: true },
    );
  })();

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
