/* clevertuna.hartle.tech
   The keyboard in the hero is the real one: same layout table the app ships,
   same idea of what a lit key looks like — opaque plastic, light escaping
   around the cap and through the legend. */

(function () {
  'use strict';

  var still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* ─────────────────────────── the sticky bar ─────────────────────── */

  var bar = document.getElementById('bar');
  var onScroll = function () {
    if (window.scrollY > 12) bar.classList.add('stuck');
    else bar.classList.remove('stuck');
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
        // Stagger anything that shares a parent, so a section arrives as one
        // movement instead of five separate ones.
        var siblings = Array.prototype.slice.call(
          e.target.parentElement.querySelectorAll(':scope > [data-reveal]'));
        var i = Math.max(0, siblings.indexOf(e.target));
        e.target.style.transitionDelay = Math.min(i * 70, 280) + 'ms';
        e.target.classList.add('in');
        seen.unobserve(e.target);
      });
    }, { rootMargin: '0px 0px -12% 0px', threshold: 0.12 });
    targets.forEach(function (el) { seen.observe(el); });
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
          duration: 1100 + Math.min(to, 200) * 3,
          easing: 'easeOutExpo',
          update: function () { el.textContent = box.n; }
        });
        countObs.unobserve(el);
      });
    }, { threshold: 0.6 });
    counters.forEach(function (el) { countObs.observe(el); });
  } else {
    counters.forEach(function (el) {
      el.textContent = el.getAttribute('data-count');
    });
  }

  /* ───────────────────────── the hero keyboard ────────────────────── */

  var canvas = document.getElementById('deck');
  var L = window.CLVX_LAYOUT;
  if (!canvas || !L) return;

  var ctx = canvas.getContext('2d', { alpha: true });
  var UW = L.u[0], UH = L.u[1], GAP = L.u[2];

  /* A run of looks, each held for a while, cross-faded into the next. Every
     one is a scheme the app actually ships. */
  var LOOKS = [
    { name: 'Colour wave · running now',
      stops: ['#00c8ff', '#8b5cf6', '#ff2fd0'], effect: 'wave',
      angle: 0, period: 2600, repeats: 1.6 },
    { name: 'Magma · welling upward',
      stops: ['#5a0000', '#ff3c00', '#ff5353', '#ffb100', '#ffe03a'], effect: 'wave',
      angle: 90, period: 2800, repeats: 1 },
    { name: 'Hartle · the house palette',
      stops: ['#36f0b1', '#00c8ff', '#8b5cf6', '#ff2fd0', '#ffb100'], effect: 'wave',
      angle: 180, period: 2200, repeats: 1.3 },
    { name: 'Tide · cyan, in and out',
      stops: ['#00c8ff'], effect: 'breathe', angle: 0, period: 3400, repeats: 1 },
    { name: 'Amber Desk · lamplight',
      stops: ['#ffb100'], effect: 'solid', angle: 0, period: 3000, repeats: 1 }
  ];

  var HOLD = 6200;   // how long each look stays
  var FADE = 1100;   // and how long it takes to become the next

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

  /* Where along the gradient a point sits. 0° is to the right and the angle
     turns anticlockwise — the same convention the app and the protocol use. */
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

  /* Which zone lights a key: the touch area and the two strips are blocks of
     the key field, not separate pads. */
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

  var dpr = 1, S = 100, W = 0, H = 0;

  function size() {
    var cssW = canvas.clientWidth || 1120;
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    S = cssW / UW;
    W = Math.round(cssW);
    H = Math.round(UH * S);
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(H * dpr);
    canvas.style.height = H + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  function rounded(x, y, w, h, r) {
    r = Math.min(r, w / 2, h / 2);
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.arcTo(x + w, y, x + w, y + h, r);
    ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r);
    ctx.arcTo(x, y, x + w, y, r);
    ctx.closePath();
  }

  var caption = document.getElementById('deck-caption-text');
  var shownCaption = -1;

  function draw(now) {
    var cycle = HOLD + FADE;
    var idx = Math.floor(now / cycle) % LOOKS.length;
    var nxt = (idx + 1) % LOOKS.length;
    var into = now % cycle;
    var blend = into > HOLD ? (into - HOLD) / FADE : 0;
    // Ease the cross-fade so a change of scheme reads as a dissolve.
    var e = blend <= 0 ? 0 : blend * blend * (3 - 2 * blend);

    if (shownCaption !== idx && caption) {
      caption.textContent = LOOKS[idx].name;
      shownCaption = idx;
    }

    ctx.clearRect(0, 0, W, H);

    // the tray
    ctx.fillStyle = '#0b0b0d';
    rounded(2, 2, W - 4, H - 4, 16);
    ctx.fill();

    var gap = GAP * S;

    // the light that gets out, as one blurred pass under the caps
    ctx.save();
    ctx.filter = 'blur(' + Math.max(2, S * 0.09) + 'px)';
    ctx.globalCompositeOperation = 'lighter';
    for (var i = 0; i < CAPS.length; i++) {
      var c = CAPS[i];
      var a = colourAt(LOOKS[idx], RGB[idx], c.nx, c.ny, now);
      var col = e > 0
        ? mix(a, colourAt(LOOKS[nxt], RGB[nxt], c.nx, c.ny, now), e)
        : a;
      var sp = S * 0.13;
      ctx.fillStyle = css(col, 0.85);
      rounded(c.x * S - sp, c.y * S - sp,
              c.w * S - gap + sp * 2, c.h * S - gap + sp * 2, S * 0.09 + sp);
      ctx.fill();
      c._col = col;
    }
    ctx.restore();

    // the keycaps: opaque plastic
    for (var j = 0; j < CAPS.length; j++) {
      var k = CAPS[j];
      ctx.fillStyle = '#151517';
      rounded(k.x * S, k.y * S, k.w * S - gap, k.h * S - gap, S * 0.055);
      ctx.fill();
      ctx.fillStyle = 'rgba(255,255,255,.07)';
      ctx.fillRect(k.x * S, k.y * S, k.w * S - gap, 1);
    }

    // the legends, glowing in the light behind them
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (var m = 0; m < CAPS.length; m++) {
      var q = CAPS[m];
      if (!q.label) continue;
      var pt = Math.max(6, Math.min(15, (q.small ? 0.30 : 0.36) * q.h * S));
      ctx.font = '600 ' + pt.toFixed(1) + 'px -apple-system, "SF Pro Text", Inter, sans-serif';
      ctx.fillStyle = css(q._col, 0.96);
      ctx.shadowColor = css(q._col, 0.8);
      ctx.shadowBlur = pt * 0.7;
      var cx = q.x * S + (q.w * S - gap) / 2;
      var cy = q.y * S + (q.h * S - gap) / 2;
      if (q.sub) {
        ctx.fillText(q.label, cx, cy - pt * 0.52);
        ctx.fillText(q.sub, cx, cy + pt * 0.58);
      } else {
        ctx.fillText(q.label, cx, cy);
      }
      ctx.shadowBlur = 0;
    }
  }

  var t0 = performance.now();
  var running = false;
  var raf = 0;

  function frame(ts) {
    draw(ts - t0);
    raf = requestAnimationFrame(frame);
  }

  function start() {
    if (running) return;
    running = true;
    raf = requestAnimationFrame(frame);
  }
  function stop() {
    running = false;
    cancelAnimationFrame(raf);
  }

  size();
  var resizeTimer;
  window.addEventListener('resize', function () {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(function () { size(); draw(performance.now() - t0); }, 120);
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
    if (document.hidden) stop();
    else if (!still) start();
  });
})();
