#!/usr/bin/env python3
"""Render docs/*.md into the site's /docs, at image build time.

The README is a landing page; the depth lives in docs/*.md; and this turns
those same files into pages so nobody has to read Markdown in a repo to find
out what a command does. One source. The site cannot drift from the thing it
describes because it is built from it.

Stdlib only — same reason the Rust side has no crates.

    python3 web/render-docs.py docs web/_docs
"""

import html
import os
import re
import sys

# Which files become pages, in nav order, with the label and blurb the index
# card carries. A file not listed here is not published: BRAND-GUIDE.md is for
# us, not for a visitor.
PAGES = [
    ("INSTALL.md", "Install", "macOS, Linux, udev rules, Windows."),
    ("USAGE.md", "Usage", "Every command, window and flag."),
    ("THEMES.md", "Themes", "The fifteen, the randomiser, the scheme file."),
    ("SAFETY.md", "Safety", "What “verified” means, and the exit codes."),
    ("PROTOCOL.md", "Protocol", "The wire format, reverse-engineered."),
    ("HARDWARE-VERIFICATION.md", "Hardware log",
     "What has actually been run against a keyboard."),
]

CSS = """
:root{--bg:#000;--ink:#f5f5f7;--dim:rgba(245,245,247,.62);
--faint:rgba(245,245,247,.42);--line:rgba(245,245,247,.12);
--card:rgba(245,245,247,.045);--card-edge:rgba(245,245,247,.09);
--cyan:#00c8ff;--mint:#36f0b1;--amber:#ffb100;--coral:#ff5353;--max:1180px}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{margin:0;background:var(--bg);color:var(--ink);
font:400 17px/1.65 -apple-system,BlinkMacSystemFont,"SF Pro Text","Inter","Helvetica Neue",Arial,sans-serif;
-webkit-font-smoothing:antialiased}
a{color:var(--cyan);text-decoration:none}
a:hover{text-decoration:underline}
img{max-width:100%;height:auto}

.bar{position:sticky;top:0;z-index:50;display:flex;align-items:center;
gap:16px;padding:13px max(22px,calc((100vw - var(--max))/2));
background:rgba(0,0,0,.72);backdrop-filter:saturate(180%) blur(18px);
border-bottom:1px solid var(--line)}
.bar .home{display:flex;align-items:center;gap:9px;color:var(--ink);font-weight:600}
.bar nav{margin-left:auto;display:flex;gap:18px;flex-wrap:wrap}
.bar nav a{color:var(--dim);font-size:14.5px}
.bar nav a:hover,.bar nav a[aria-current]{color:var(--ink);text-decoration:none}

.wrap{max-width:var(--max);margin:0 auto;padding:0 22px;
display:grid;grid-template-columns:212px minmax(0,1fr);gap:52px}
@media(max-width:860px){.wrap{grid-template-columns:1fr;gap:0}
aside{position:static!important;padding:22px 0 0!important;border:0!important}}

aside{position:sticky;top:74px;align-self:start;padding:44px 0;
max-height:calc(100vh - 74px);overflow:auto}
aside h2{font-size:11px;letter-spacing:.14em;text-transform:uppercase;
color:var(--faint);margin:0 0 12px}
aside ul{list-style:none;margin:0 0 26px;padding:0}
aside li{margin:0 0 3px}
aside a{display:block;padding:5px 11px;margin-left:-11px;border-radius:8px;
color:var(--dim);font-size:15px}
aside a:hover{background:var(--card);color:var(--ink);text-decoration:none}
aside a.on{background:var(--card);color:var(--ink);
box-shadow:inset 2px 0 0 var(--cyan)}
aside .sub a{font-size:14px;padding-left:22px;color:var(--faint)}

main{padding:44px 0 96px;min-width:0}
main>h1{font-size:clamp(30px,4.4vw,44px);line-height:1.1;letter-spacing:-.02em;
margin:0 0 28px;background:linear-gradient(100deg,var(--ink),var(--cyan) 62%,var(--mint));
-webkit-background-clip:text;background-clip:text;color:transparent}
main h2{font-size:25px;letter-spacing:-.01em;margin:52px 0 14px;
padding-top:18px;border-top:1px solid var(--line)}
main h3{font-size:19px;margin:34px 0 10px}
main p{margin:0 0 16px;color:var(--dim);max-width:70ch}
main strong{color:var(--ink)}
main li{color:var(--dim);margin:0 0 7px;max-width:70ch}
main ul,main ol{padding-left:22px}

code{font:500 13.5px/1.5 ui-monospace,SFMono-Regular,"SF Mono",Menlo,monospace;
background:var(--card);border:1px solid var(--card-edge);border-radius:6px;
padding:.12em .4em;color:var(--mint)}
pre{background:#07101f;border:1px solid var(--card-edge);border-radius:13px;
padding:17px 19px;overflow-x:auto;margin:0 0 20px}
pre code{background:0;border:0;padding:0;color:#cfe9ff;font-size:13.5px;line-height:1.62}

table{border-collapse:collapse;width:100%;margin:0 0 22px;font-size:15px;
display:block;overflow-x:auto}
th,td{text-align:left;padding:9px 14px;border-bottom:1px solid var(--line);
vertical-align:top;color:var(--dim)}
th{color:var(--faint);font-size:11.5px;letter-spacing:.1em;text-transform:uppercase;
font-weight:600;white-space:nowrap}
td strong{color:var(--ink)}

blockquote{margin:0 0 20px;padding:14px 19px;border-left:3px solid var(--amber);
background:var(--card);border-radius:0 11px 11px 0}
blockquote p{margin:0;color:var(--ink)}
hr{border:0;border-top:1px solid var(--line);margin:36px 0}

.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(248px,1fr));
gap:15px;margin:0 0 30px;padding:0;list-style:none}
.cards a{display:block;height:100%;padding:19px 21px;border-radius:15px;
background:var(--card);border:1px solid var(--card-edge);color:var(--ink)}
.cards a:hover{border-color:var(--cyan);text-decoration:none;
background:rgba(0,200,255,.06)}
.cards b{display:block;font-size:17px;margin:0 0 5px}
.cards span{color:var(--dim);font-size:14.5px;line-height:1.5}

footer{border-top:1px solid var(--line);padding:26px 0 0;margin-top:60px;
color:var(--faint);font-size:14px}
footer a{color:var(--dim)}
"""


def slug(text):
    """GitHub's anchor rule, near enough: lowercase, strip punctuation,
    spaces to hyphens. Keeps in-page links working across both renderings."""
    s = re.sub(r"<[^>]+>", "", text).lower()
    s = re.sub(r"[^\w\s-]", "", s, flags=re.UNICODE)
    return re.sub(r"[\s-]+", "-", s).strip("-")


def inline(text):
    """Inline markdown. Code spans are pulled out first so nothing formats
    inside them — otherwise `--no-color` grows an <em>."""
    spans = []

    def stash(m):
        spans.append(m.group(1))
        return "\x00%d\x00" % (len(spans) - 1)

    text = re.sub(r"`([^`]+)`", stash, text)
    text = html.escape(text, quote=False)
    text = re.sub(r"!\[([^\]]*)\]\(([^)]+)\)",
                  lambda m: '<img src="%s" alt="%s">' % (m.group(2), m.group(1)), text)
    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<![\w*])\*([^*\n]+)\*(?![\w*])", r"<em>\1</em>", text)
    text = re.sub(r"\x00(\d+)\x00",
                  lambda m: "<code>%s</code>" % html.escape(spans[int(m.group(1))]), text)
    return text


def link(m):
    label, href = m.group(1), m.group(2)
    # A sibling .md becomes its rendered page; ../ escapes docs/ entirely and
    # can only sensibly point at the repository.
    if href.startswith("../"):
        href = "https://github.com/hartle-tech/clevertuna/blob/main/" + href[3:]
    elif href.endswith(".md") or ".md#" in href:
        name, _, frag = href.partition("#")
        href = name[:-3].lower() + (".html#" + frag if frag else ".html")
    return '<a href="%s">%s</a>' % (html.escape(href, quote=True), label)


def render(md):
    """Markdown → (html, [(level, text, anchor)])."""
    out, toc, lines, i = [], [], md.split("\n"), 0
    while i < len(lines):
        ln = lines[i]

        if ln.startswith("```"):
            body, i = [], i + 1
            while i < len(lines) and not lines[i].startswith("```"):
                body.append(lines[i])
                i += 1
            out.append("<pre><code>%s</code></pre>"
                       % html.escape("\n".join(body), quote=False))
            i += 1
            continue

        m = re.match(r"(#{1,4})\s+(.*)", ln)
        if m:
            lvl, text = len(m.group(1)), m.group(2).strip()
            a = slug(text)
            if lvl in (2, 3):
                toc.append((lvl, text, a))
            out.append('<h%d id="%s">%s</h%d>' % (lvl, a, inline(text), lvl))
            i += 1
            continue

        if ln.startswith("|") and i + 1 < len(lines) and re.match(r"^\|[\s:|-]+\|$", lines[i + 1]):
            head = [c.strip() for c in ln.strip("|").split("|")]
            i += 2
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append([c.strip() for c in lines[i].strip("|").split("|")])
                i += 1
            th = "".join("<th>%s</th>" % inline(c) for c in head)
            tb = "".join("<tr>%s</tr>" % "".join("<td>%s</td>" % inline(c) for c in r)
                         for r in rows)
            out.append("<table><thead><tr>%s</tr></thead><tbody>%s</tbody></table>" % (th, tb))
            continue

        if re.match(r"^\s*[-*]\s+|^\s*\d+\.\s+", ln):
            ordered = bool(re.match(r"^\s*\d+\.", ln))
            items = []
            while i < len(lines) and (re.match(r"^\s*[-*]\s+|^\s*\d+\.\s+", lines[i])
                                      or (lines[i].startswith("  ") and lines[i].strip() and items)):
                if re.match(r"^\s*[-*]\s+|^\s*\d+\.\s+", lines[i]):
                    items.append(re.sub(r"^\s*(?:[-*]|\d+\.)\s+", "", lines[i]))
                else:                       # a wrapped continuation line
                    items[-1] += " " + lines[i].strip()
                i += 1
            tag = "ol" if ordered else "ul"
            out.append("<%s>%s</%s>" % (tag, "".join("<li>%s</li>" % inline(x)
                                                    for x in items), tag))
            continue

        if ln.startswith(">"):
            body = []
            while i < len(lines) and lines[i].startswith(">"):
                body.append(lines[i].lstrip(">").strip())
                i += 1
            out.append("<blockquote><p>%s</p></blockquote>" % inline(" ".join(body)))
            continue

        if ln.strip() in ("---", "***", "___"):
            out.append("<hr>")
            i += 1
            continue

        if not ln.strip():
            i += 1
            continue

        para = []
        while i < len(lines) and lines[i].strip() and not re.match(
                r"^(#{1,4}\s|```|\||>|\s*[-*]\s+|\s*\d+\.\s+|---$)", lines[i]):
            para.append(lines[i].strip())
            i += 1
        out.append("<p>%s</p>" % inline(" ".join(para)))

    return "\n".join(out), toc


def shell(title, nav, aside, body, blurb=""):
    return """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>%s — Clevertuna docs</title>
<meta name="description" content="%s">
<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" href="/mark.svg" type="image/svg+xml" sizes="any">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<!-- A real stylesheet, not a <style> block: the site's CSP is
     `style-src 'self'` with no 'unsafe-inline', and an inline block is
     dropped silently — the page renders, entirely unstyled. -->
<link rel="stylesheet" href="/docs/docs.css">
</head>
<body>
<header class="bar">
  <a class="home" href="/">
    <svg viewBox="0 0 32 32" width="21" height="21" aria-hidden="true">
      <defs><linearGradient id="m" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#00c8ff"/><stop offset="1" stop-color="#36f0b1"/>
      </linearGradient></defs>
      <rect width="32" height="32" rx="8" fill="url(#m)"/>
      <path d="M5 20.5 C8.5 20.5 8.5 11.5 12.5 11.5 S16.5 20.5 20 20.5 S27 11.5 27 11.5"
            fill="none" stroke="#04121a" stroke-width="4.6"
            stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
    Clevertuna
  </a>
  <nav>%s</nav>
</header>
<div class="wrap">
<aside>%s</aside>
<main>
%s
<footer>Apache-2.0 · © HARTLE.TECH ·
<a href="mailto:contact@hartle.tech">contact@hartle.tech</a> ·
<a href="https://github.com/hartle-tech/clevertuna">GitHub</a> ·
<a href="/#support">Support this work</a></footer>
</main>
</div>
</body>
</html>
""" % (html.escape(title), html.escape(blurb or title), nav, aside, body)


def main(src, dst):
    os.makedirs(dst, exist_ok=True)
    open(os.path.join(dst, "docs.css"), "w", encoding="utf-8").write(CSS.strip() + "\n")
    nav = ('<a href="/docs/">Docs</a><a href="/#features">Features</a>'
           '<a href="/#compare">Compare</a>'
           '<a href="https://github.com/hartle-tech/clevertuna">GitHub</a>'
           '<a href="/#support">Support</a>')

    def sidebar(current, toc=()):
        """The page list, plus this page's own H2s under it."""
        items = "".join(
            '<li><a class="%s" href="%s.html">%s</a></li>'
            % ("on" if f == current else "", f[:-3].lower(), t)
            for f, t, _ in PAGES)
        out = "<h2>Docs</h2><ul>%s</ul>" % items
        heads = [(t, a) for lvl, t, a in toc if lvl == 2]
        if heads:
            out += '<h2>On this page</h2><ul class="sub">%s</ul>' % "".join(
                '<li><a href="#%s">%s</a></li>' % (a, html.escape(t)) for t, a in heads)
        return out

    written = []
    for fname, title, blurb in PAGES:
        path = os.path.join(src, fname)
        if not os.path.exists(path):
            print("  skip (absent) %s" % fname)
            continue
        body, toc = render(open(path, encoding="utf-8").read())
        out = os.path.join(dst, fname[:-3].lower() + ".html")
        open(out, "w", encoding="utf-8").write(
            shell(title, nav, sidebar(fname, toc), body, blurb))
        written.append((fname, title, blurb))
        print("  %-28s -> %s" % (fname, os.path.basename(out)))

    cards = "".join(
        '<li><a href="%s.html"><b>%s</b><span>%s</span></a></li>'
        % (f[:-3].lower(), html.escape(t), html.escape(b)) for f, t, b in written)
    index = shell("Docs", nav, sidebar(None),
                  "<h1>Clevertuna docs</h1>"
                  "<p>Everything the README deliberately does not say. "
                  "The same files live in "
                  '<a href="https://github.com/hartle-tech/clevertuna/tree/main/docs">'
                  "<code>docs/</code></a> in the repository.</p>"
                  '<ul class="cards">%s</ul>' % cards,
                  "Every command, the wire format, and what has been run against real hardware.")
    open(os.path.join(dst, "index.html"), "w", encoding="utf-8").write(index)
    print("  %-28s -> index.html, docs.css" % "(index)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "docs",
                  sys.argv[2] if len(sys.argv) > 2 else "web/_docs"))
