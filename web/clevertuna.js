/* clevertuna.hartle.tech
   The keyboard in the hero is the real one: same layout table the app ships,
   same idea of what a lit key looks like — opaque plastic, light escaping
   around the cap and through the legend.

   What costs nothing per frame, doesn't happen per frame. The keycaps and the
   legend shapes never change, so they are drawn once into offscreen canvases
   and blitted; the glow is drawn at half resolution because it is a blur and
   nobody can tell; and the legends are tinted by compositing the colour field
   through a mask rather than by re-laying-out eighty-one pieces of text with a
   shadow on each. That last one was most of the cost. */

(function () {
  'use strict';

  var still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* ─────────────────────────── the sticky bar ─────────────────────── */

  var bar = document.getElementById('bar');
  var stuck = false;
  var onScroll = function () {
    var want = window.scrollY > 12;
    if (want !== stuck) { stuck = want; bar.classList.toggle('stuck', want); }
  };
  onScroll();
  window.addEventListener('scroll', onScroll, { passive: true });

  /* ───────────────────────── reveal on scroll ─────────────────────── */

  var targets = document.querySelectorAll('[data-reveal]');
  if (still || !('IntersectionObserver' in window)) {
    targets.forEach(function (el) { el.classList.add('in'); });
  } else {
    var seen = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (!e.isIntersecting) return;
        var el = e.target;
        var i = +(el.dataset.revealIndex || 0);
        el.style.transitionDelay = Math.min(i * 70, 280) + 'ms';
        el.classList.add('in');
        seen.unobserve(el);
      });
    }, { rootMargin: '0px 0px -10% 0px', threshold: 0.1 });
    // Index once, up front: querying siblings inside the callback made every
    // intersection walk the DOM again.
    targets.forEach(function (el) {
      var sibs = el.parentElement.querySelectorAll(':scope > [data-reveal]');
      el.dataset.revealIndex = Math.max(0, Array.prototype.indexOf.call(sibs, el));
      seen.observe(el);
    });
  }

  /* ───────────────────────────── counters ─────────────────────────── */

  var counters = document.querySelectorAll('[data-count]');
  if (counters.length && !still && window.anime) {
    var countObs = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (!e.isIntersecting) return;
        var el = e.target;
        var to = parseInt(el.getAttribute('data-count'), 10) || 0;
        var box = { n: 0 };
        window.anime({
          targets: box, n: to, round: 1,
          duration: 1000 + Math.min(to, 200) * 3,
          easing: 'easeOutExpo',
          update: function () { el.textContent = box.n; }
        });
        countObs.unobserve(el);
      });
    }, { threshold: 0.6 });
    counters.forEach(function (el) { countObs.observe(el); });
  } else {
    counters.forEach(function (el) { el.textContent = el.getAttribute('data-count'); });
  }

  /* ───────────────────────── the hero keyboard ────────────────────── */

  var canvas = document.getElementById('deck');
  var L = window.CLVX_LAYOUT;
  if (!canvas || !L) return;

  var ctx = canvas.getContext('2d', { alpha: true });
  var UW = L.u[0], UH = L.u[1], GAP = L.u[2];

  var LOOKS = [
    { name: 'Hartle · the house palette',
      stops: ['#00c8ff', '#36f0b1', '#ff00e8', '#ff5353', '#ffb100'],
      effect: 'wave', angle: 0, period: 2600, repeats: 1.4 },
    { name: 'Magma · welling upward',
      stops: ['#5a0000', '#ff3c00', '#ff5353', '#ffb100', '#ffe03a'],
      effect: 'wave', angle: 90, period: 2800, repeats: 1 },
    { name: 'Spectrum · the whole wheel',
      stops: ['#ff5353', '#ffb100', '#ffe03a', '#00e07a', '#00c8ff', '#8b5cf6'],
      effect: 'wave', angle: 0, period: 2400, repeats: 1.2 },
    { name: 'Tide · cyan, in and out',
      stops: ['#00c8ff'], effect: 'breathe', angle: 0, period: 3400, repeats: 1 },
    { name: 'Amber Desk · lamplight',
      stops: ['#ffb100'], effect: 'solid', angle: 0, period: 3000, repeats: 1 }
  ];

  var HOLD = 6200, FADE = 1100;

  function hex(c) {
    return [parseInt(c.slice(1, 3), 16), parseInt(c.slice(3, 5), 16), parseInt(c.slice(5, 7), 16)];
  }
  var RGB = LOOKS.map(function (l) { return l.stops.map(hex); });

  function mix(a, b, t) {
    return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  }
  function ramp(list, u) {
    if (list.length === 1) return list[0];
    u = u - Math.floor(u);
    var s = u * list.length, i = Math.floor(s) % list.length;
    return mix(list[i], list[(i + 1) % list.length], s - i);
  }
  function css(c, a) {
    return 'rgba(' + (c[0] | 0) + ',' + (c[1] | 0) + ',' + (c[2] | 0) + ',' + a + ')';
  }

  /* 0° is to the right and the angle turns anticlockwise — the convention the
     app and the protocol use. */
  function axis(look, nx, ny) {
    var r = look.angle * Math.PI / 180;
    return 0.5 + ((nx - 0.5) * Math.cos(r) - (ny - 0.5) * Math.sin(r));
  }

  function colourAt(look, list, nx, ny, t) {
    var phase = (t % look.period) / look.period;
    if (look.effect === 'solid') return list[0];
    if (look.effect === 'breathe') {
      var b = 0.35 + 0.65 * (0.5 + 0.5 * Math.cos(phase * 2 * Math.PI));
      return [list[0][0] * b, list[0][1] * b, list[0][2] * b];
    }
    return ramp(list, axis(look, nx, ny) * look.repeats - phase);
  }

  var Z = L.z;
  function inside(r, x, y) {
    return x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3];
  }

  var CAPS = L.k.map(function (k) {
    var cx = k[0] + k[2] / 2, cy = k[1] + k[3] / 2;
    var zone = null;
    ['touchpad', 'leftSlider', 'rightSlider'].forEach(function (id) {
      if (!zone && inside(Z[id], cx, cy)) zone = id;
    });
    var r = zone ? Z[zone] : null;
    return {
      x: k[0], y: k[1], w: k[2], h: k[3],
      label: k[4], sub: k[5], small: k[6],
      nx: r ? (cx - r[0]) / r[2] : cx / UW,
      ny: r ? (cy - r[1]) / r[3] : cy / UH
    };
  });

  function makeCanvas(w, h) {
    var c = document.createElement('canvas');
    c.width = Math.max(1, Math.round(w));
    c.height = Math.max(1, Math.round(h));
    return c;
  }

  function rounded(g, x, y, w, h, r) {
    r = Math.min(r, w / 2, h / 2);
    g.beginPath();
    g.moveTo(x + r, y);
    g.arcTo(x + w, y, x + w, y + h, r);
    g.arcTo(x + w, y + h, x, y + h, r);
    g.arcTo(x, y + h, x, y, r);
    g.arcTo(x, y, x + w, y, r);
    g.closePath();
  }

  /* Everything that depends only on size, built once per resize. */
  var dpr = 1, S = 100, W = 0, H = 0;
  var capsLayer, legendMask, tintLayer, glowLayer, glowScale = 0.5;

  function build() {
    var cssW = canvas.clientWidth || 1120;
    // Two is plenty for a drawing of a keyboard, and three is a third more
    // pixels for nothing.
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    S = cssW / UW;
    W = Math.round(cssW);
    H = Math.round(UH * S);
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(H * dpr);
    canvas.style.height = H + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    var pw = W * dpr, ph = H * dpr;
    var gap = GAP * S;

    // ── the keycaps: opaque plastic, drawn once ──────────────────────
    capsLayer = makeCanvas(pw, ph);
    var cg = capsLayer.getContext('2d');
    cg.setTransform(dpr, 0, 0, dpr, 0, 0);
    cg.fillStyle = '#0b0b0d';
    rounded(cg, 2, 2, W - 4, H - 4, 16);
    cg.fill();
    CAPS.forEach(function (c) {
      cg.fillStyle = '#151517';
      rounded(cg, c.x * S, c.y * S, c.w * S - gap, c.h * S - gap, S * 0.055);
      cg.fill();
      cg.fillStyle = 'rgba(255,255,255,.07)';
      cg.fillRect(c.x * S, c.y * S, c.w * S - gap, 1);
    });

    // ── the legends, as a white mask, laid out once ──────────────────
    legendMask = makeCanvas(pw, ph);
    var lg = legendMask.getContext('2d');
    lg.setTransform(dpr, 0, 0, dpr, 0, 0);
    lg.textAlign = 'center';
    lg.textBaseline = 'middle';
    lg.fillStyle = '#fff';
    CAPS.forEach(function (c) {
      if (!c.label) return;
      var pt = Math.max(6, Math.min(15, (c.small ? 0.30 : 0.36) * c.h * S));
      lg.font = '600 ' + pt.toFixed(1) + 'px -apple-system, "SF Pro Text", Inter, sans-serif';
      var mx = c.x * S + (c.w * S - gap) / 2;
      var my = c.y * S + (c.h * S - gap) / 2;
      if (c.sub) {
        lg.fillText(c.label, mx, my - pt * 0.52);
        lg.fillText(c.sub, mx, my + pt * 0.58);
      } else {
        lg.fillText(c.label, mx, my);
      }
    });

    // Scratch buffers reused every frame.
    tintLayer = makeCanvas(pw, ph);
    glowLayer = makeCanvas(pw * glowScale, ph * glowScale);
  }

  var caption = document.getElementById('deck-caption-text');
  var shownCaption = -1;

  function draw(now) {
    var cycle = HOLD + FADE;
    var idx = Math.floor(now / cycle) % LOOKS.length;
    var nxt = (idx + 1) % LOOKS.length;
    var into = now % cycle;
    var b = into > HOLD ? (into - HOLD) / FADE : 0;
    var e = b <= 0 ? 0 : b * b * (3 - 2 * b);

    if (shownCaption !== idx && caption) {
      caption.textContent = LOOKS[idx].name;
      shownCaption = idx;
    }

    var gap = GAP * S;
    var i, c, col;

    // Resolve each key's colour once for the whole frame.
    for (i = 0; i < CAPS.length; i++) {
      c = CAPS[i];
      col = colourAt(LOOKS[idx], RGB[idx], c.nx, c.ny, now);
      c._col = e > 0 ? mix(col, colourAt(LOOKS[nxt], RGB[nxt], c.nx, c.ny, now), e) : col;
    }

    // ── the glow, at half resolution because it is a blur ────────────
    var gg = glowLayer.getContext('2d');
    gg.setTransform(dpr * glowScale, 0, 0, dpr * glowScale, 0, 0);
    gg.clearRect(0, 0, W, H);
    gg.filter = 'blur(' + Math.max(1.5, S * 0.075) + 'px)';
    gg.globalCompositeOperation = 'source-over';
    var sp = S * 0.13;
    for (i = 0; i < CAPS.length; i++) {
      c = CAPS[i];
      gg.fillStyle = css(c._col, 0.85);
      rounded(gg, c.x * S - sp, c.y * S - sp,
              c.w * S - gap + sp * 2, c.h * S - gap + sp * 2, S * 0.09 + sp);
      gg.fill();
    }
    gg.filter = 'none';

    // ── the legends: colour field masked by the pre-laid-out text ────
    var tg = tintLayer.getContext('2d');
    tg.setTransform(dpr, 0, 0, dpr, 0, 0);
    tg.globalCompositeOperation = 'source-over';
    tg.clearRect(0, 0, W, H);
    for (i = 0; i < CAPS.length; i++) {
      c = CAPS[i];
      tg.fillStyle = css(c._col, 1);
      tg.fillRect(c.x * S - 1, c.y * S - 1, c.w * S - gap + 2, c.h * S - gap + 2);
    }
    tg.globalCompositeOperation = 'destination-in';
    tg.setTransform(1, 0, 0, 1, 0, 0);
    tg.drawImage(legendMask, 0, 0);

    // ── compose ──────────────────────────────────────────────────────
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(glowLayer, 0, 0, canvas.width, canvas.height);
    ctx.drawImage(capsLayer, 0, 0);
    ctx.drawImage(tintLayer, 0, 0);
  }

  var t0 = performance.now();
  var running = false, raf = 0;

  function frame(ts) {
    draw(ts - t0);
    raf = requestAnimationFrame(frame);
  }
  function start() { if (!running) { running = true; raf = requestAnimationFrame(frame); } }
  function stop() { running = false; cancelAnimationFrame(raf); }

  build();

  var resizeTimer, lastW = canvas.clientWidth;
  window.addEventListener('resize', function () {
    if (canvas.clientWidth === lastW) return;   // height-only changes are noise
    lastW = canvas.clientWidth;
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(function () {
      build();
      draw(performance.now() - t0);
    }, 140);
  }, { passive: true });

  if (still) {
    draw(1500);
  } else if ('IntersectionObserver' in window) {
    // Off screen, it stops: an animation nobody is looking at is a fan noise.
    new IntersectionObserver(function (entries) {
      entries.forEach(function (e) { e.isIntersecting ? start() : stop(); });
    }, { threshold: 0.02 }).observe(canvas);
  } else {
    start();
  }

  document.addEventListener('visibilitychange', function () {
    if (document.hidden) stop(); else if (!still) start();
  });
})();
