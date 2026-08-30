// Turn the .dc.html artboards into one plain page.
//
// The design canvas mounts every artboard in a sandboxed iframe that has to
// answer the editor within 1.5 s of loading. On a busy machine that budget is
// missed and the artboard shows "Preview stopped" instead of the design. This
// renderer resolves the same sources ahead of time — running each board's
// renderVals() and expanding its sc-for loops and {{holes}} — so the design
// can be read as ordinary HTML, with nothing to hand-shake with.
//
// Usage: node render-static.mjs > clevertuna-redesign.html

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

const BOARDS = [
  { file: 'Main.dc.html', title: 'Theme Builder',
    note: 'Panes of glass floating over the keyboard’s own light — a toolbar, an inspector, and the deck itself as the window’s content. Nothing is written to the keyboard until you apply.' },
  { file: 'MenuBar.dc.html', title: 'Helper menu',
    note: 'The quick surface, kept. Brightness, six themes as tiles, and the two smart ones — every row one tap, each showing the key that does the same without opening anything. Only Builder and Settings open a window, which is why they sit apart at the bottom.' },
  { file: 'Themes.dc.html', title: 'Themes',
    note: 'The selected theme is the window’s backdrop, so every pane of glass on top of it is tinted by the light it describes. The card carries only what you cannot see by looking.' },
  { file: 'Settings.dc.html', title: 'Settings',
    note: 'Everything that is not a theme, read against a pale surface: the same material, the same capsules. What this keyboard cannot do is shown and disabled rather than hidden.' },
  { file: 'Kit.dc.html', title: 'The material',
    note: 'Glass built from three painted layers — body, specular rim, lift — over content that is genuinely there. No backdrop-filter: the content behind the glass is real, so a live blur would cost paint without changing the reading.' },
];

// --- the little bit of the DC runtime these boards actually use -------------

class DCLogic {
  constructor(props) { this.props = props; }
}

function propsOf(src) {
  const m = src.match(/data-props='([^']*)'/);
  if (!m) return {};
  const spec = JSON.parse(m[1]);
  return Object.fromEntries(Object.entries(spec).map(([k, v]) => [k, v.default]));
}

function valsOf(src) {
  const body = src.match(/<script data-dc-script[^>]*>([\s\S]*?)<\/script>/);
  if (!body) return {};
  const make = new Function('DCLogic', `${body[1]}; return Component;`);
  return new (make(DCLogic))(propsOf(src)).renderVals();
}

// --- template expansion -----------------------------------------------------

// A scope is a chain of {name: value} frames; "s.bg" reads s from the nearest
// frame that has it. resolve() hands back the value itself — sc-for needs the
// array, not a rendering of it — and only fill() turns one into text.
function resolve(path, scope) {
  const [head, ...rest] = path.split('.');
  let cur;
  for (let i = scope.length - 1; i >= 0; i--) {
    if (head in scope[i]) { cur = scope[i][head]; break; }
  }
  for (const key of rest) cur = cur == null ? undefined : cur[key];
  return cur;
}

// Missing holes resolve to '' rather than printing "{{x}}".
const fill = (text, scope) =>
  text.replace(/\{\{\s*([\w.]+)\s*\}\}/g, (_, p) => {
    const v = resolve(p, scope);
    return v == null ? '' : String(v);
  });

// Expand the innermost sc-for first, so nesting falls out of repeated passes.
function expand(html, scope) {
  const open = /<sc-for\s+list="\{\{\s*([\w.]+)\s*\}\}"\s+as="(\w+)"[^>]*>/;
  for (;;) {
    const m = open.exec(html);
    if (!m) break;
    // Walk forward from this tag to its matching close, counting nesting.
    let depth = 1;
    let i = m.index + m[0].length;
    const start = i;
    while (depth > 0) {
      const next = /<sc-for\b[^>]*>|<\/sc-for>/g;
      next.lastIndex = i;
      const t = next.exec(html);
      if (!t) throw new Error('unbalanced sc-for');
      depth += t[0] === '</sc-for>' ? -1 : 1;
      i = t.index + t[0].length;
      if (depth === 0) {
        const inner = html.slice(start, t.index);
        const list = resolve(m[1], scope);
        const items = Array.isArray(list) ? list : [];
        const out = items
          .map((it) => expand(inner, [...scope, { [m[2]]: it }]))
          .join('');
        html = html.slice(0, m.index) + out + html.slice(i);
      }
    }
  }
  return fill(html, scope);
}

// --- scoping the per-board CSS ---------------------------------------------

// Every board names its classes the same (.glass, .tile) with different values,
// so each board's rules are confined to its own wrapper.
function scopeCss(css, sel) {
  return css.replace(/(^|\})\s*([^{}@]+)\{/g, (all, close, selectors) => {
    const scoped = selectors
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) => (s === 'body' || s === 'html' ? sel : `${sel} ${s}`))
      .join(', ');
    return `${close}\n${scoped} {`;
  });
}

// --- build ------------------------------------------------------------------

const sections = BOARDS.map((b, n) => {
  const src = readFileSync(join(here, b.file), 'utf8');
  const sel = `#board-${n}`;
  const css = (src.match(/<helmet>\s*<style>([\s\S]*?)<\/style>/) || ['', ''])[1];
  const markup = src.match(/<x-dc>([\s\S]*?)<\/x-dc>/)[1]
    .replace(/<helmet>[\s\S]*?<\/helmet>/, '');
  return {
    ...b,
    css: scopeCss(css, sel),
    html: expand(markup, [valsOf(src)]),
    id: `board-${n}`,
  };
});

const page = `<title>Clevertuna Redesign</title>
<style>
  /* The page commits to one visual world: these are macOS surfaces, shown in
     the system's own face against a ground dark enough that the keyboard's
     light is the only colour on the page. Every colour is painted, so the page
     holds whichever theme the viewer is in. */
  :root {
    color-scheme: dark;
    --ink: #F5F5F7;
    --ink-2: rgba(245,245,247,0.70);
    --ink-3: rgba(245,245,247,0.48);
    --ground: #0A0A0C;
    --rule: rgba(245,245,247,0.10);
  }
  * { box-sizing: border-box; }
  body {
    margin: 0; background: var(--ground); color: var(--ink);
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif;
    font-size: 14px; line-height: 1.55; -webkit-font-smoothing: antialiased;
  }
  .page { max-width: 1220px; margin: 0 auto; padding: 80px 24px 128px; }
  .lede { max-width: 60ch; }
  h1 {
    font-size: clamp(30px, 4.4vw, 42px); line-height: 1.08; font-weight: 680;
    letter-spacing: -0.028em; margin: 0 0 16px; text-wrap: balance;
  }
  .sub { color: var(--ink-2); font-size: 16px; margin: 0; }
  section { margin-top: 88px; padding-top: 30px; border-top: 0.5px solid var(--rule); }
  h2 {
    font-size: 12px; font-weight: 590; letter-spacing: 0.07em; text-transform: uppercase;
    color: var(--ink-3); margin: 0 0 8px;
  }
  .note { max-width: 62ch; color: var(--ink-2); margin: 0 0 26px; }
  .stage { display: flex; overflow-x: auto; padding-bottom: 8px; }
  .stage > * { flex: none; }
${sections.map((s) => s.css).join('\n')}
</style>

<div class="page">
  <div class="lede">
    <h1>Clevertuna for macOS</h1>
    <p class="sub">The keyboard app, redrawn in the macOS 26 material: capsule
    controls, concentric radii, and panels that float over the content rather
    than sitting beside it. The glass is tinted by whatever it is over, which
    for this app is the keyboard’s own light.</p>
  </div>

${sections.map((s) => `  <section>
    <h2>${s.title}</h2>
    <p class="note">${s.note}</p>
    <div class="stage"><div id="${s.id}">${s.html}</div></div>
  </section>`).join('\n\n')}
</div>
`;

process.stdout.write(page);
