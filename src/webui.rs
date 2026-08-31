//! The local UI: one embedded page, served on loopback, in the same binary.
//!
//! Deliberately small and deliberately boring about security. It binds to
//! 127.0.0.1, serves exactly one document plus three JSON endpoints, never
//! takes a filesystem path from the client, caps request bodies, and handles
//! one request at a time so two tabs cannot interleave hardware exchanges.
//!
//! There are no accounts, no cloud, no analytics, and no outbound requests.

use crate::json::{self, Json};
use crate::service::{self, Stage};
use crate::transport::Device;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};

const MAX_BODY: usize = 256 * 1024;

pub fn serve(dev: &mut Device, port: u16, open_hint: bool) -> i32 {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot bind 127.0.0.1:{}: {}", port, e);
            return 4;
        }
    };
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_default();
    if open_hint {
        println!("CLEVERTUNA  http://{}  (loopback only — ctrl-c to stop)", bound);
    }
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if handle(dev, s).is_err() {
                    continue;
                }
            }
            Err(_) => continue,
        }
    }
    0
}

fn handle(dev: &mut Device, mut stream: TcpStream) -> std::io::Result<()> {
    let peer_is_local = stream
        .peer_addr()
        .map(|a| a.ip().is_loopback())
        .unwrap_or(false);
    if !peer_is_local {
        // belt and braces: we only bound loopback, but never serve anything else
        return respond(&mut stream, 403, "text/plain", b"forbidden");
    }

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    if content_length > MAX_BODY {
        return respond(&mut stream, 413, "application/json",
                       br#"{"stage":"failed","message":"request too large"}"#);
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    match (method.as_str(), path.as_str()) {
        ("GET", "/") => respond(&mut stream, 200, "text/html; charset=utf-8", PAGE.as_bytes()),
        ("GET", "/api/state") => {
            let payload = state_json(dev);
            respond(&mut stream, 200, "application/json", json::to_string_pretty(&payload).as_bytes())
        }
        ("POST", "/api/apply") => {
            let text = String::from_utf8_lossy(&body).to_string();
            let payload = match json::parse(&text) {
                Ok(doc) => apply_json(dev, &doc),
                Err(e) => Json::obj(vec![
                    ("stage", Json::Str("failed".into())),
                    ("message", Json::Str(format!("invalid scheme: {}", e))),
                ]),
            };
            respond(&mut stream, 200, "application/json", json::to_string_pretty(&payload).as_bytes())
        }
        ("POST", "/api/backup") => {
            let payload = match service::get_settings(dev) {
                Ok(blob) => {
                    // the client never chooses a path
                    let name = "clevertuna-backup.clvx";
                    match std::fs::write(name, &blob) {
                        Ok(_) => Json::obj(vec![
                            ("stage", Json::Str("read_back".into())),
                            ("file", Json::Str(name.into())),
                            ("bytes", Json::Num(blob.len() as f64)),
                        ]),
                        Err(e) => Json::obj(vec![
                            ("stage", Json::Str("failed".into())),
                            ("message", Json::Str(format!("cannot write backup: {}", e))),
                        ]),
                    }
                }
                Err(e) => Json::obj(vec![
                    ("stage", Json::Str("failed".into())),
                    ("message", Json::Str(format!("{}", e))),
                ]),
            };
            respond(&mut stream, 200, "application/json", json::to_string_pretty(&payload).as_bytes())
        }
        _ => respond(&mut stream, 404, "application/json",
                     br#"{"stage":"failed","message":"no such endpoint"}"#),
    }
}

pub fn state_json(dev: &mut Device) -> Json {
    match service::get_backlight_json(dev) {
        Ok(doc) => Json::obj(vec![
            ("stage", Json::Str("read_back".into())),
            ("transport", Json::Str(dev.kind.label().into())),
            ("connected", Json::Bool(true)),
            ("backlight", doc.get("backlight").cloned().unwrap_or(Json::Null)),
        ]),
        Err(e) => Json::obj(vec![
            ("stage", Json::Str("failed".into())),
            ("transport", Json::Str(dev.kind.label().into())),
            ("connected", Json::Bool(false)),
            ("message", Json::Str(format!("{}", e))),
        ]),
    }
}

pub fn apply_json(dev: &mut Device, doc: &Json) -> Json {
    match service::set_backlight_verified(dev, doc) {
        Ok(out) => {
            let mut pairs = vec![
                ("stage", Json::Str(out.stage.label().into())),
                ("message", Json::Str(out.message.clone())),
                ("transport", Json::Str(dev.kind.label().into())),
            ];
            if out.stage == Stage::Mismatch {
                if let Some(e) = out.expected {
                    pairs.push(("expected", e));
                }
                if let Some(a) = out.actual {
                    pairs.push(("actual", a));
                }
            }
            Json::obj(pairs)
        }
        Err(e) => Json::obj(vec![
            ("stage", Json::Str("failed".into())),
            ("message", Json::Str(format!("{}", e))),
        ]),
    }
}

fn respond(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        code, reason, ctype, body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// The whole interface. Brand tokens inline so the binary needs no asset files.
const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Clevertuna</title>
<style>
:root{
  --abyss:#1C1949; --ink:#07101F; --reef:#0096FF; --current:#00C8FF;
  --mint:#36F0B1; --trench:#FF00E8; --coral:#FF5353; --amber:#FFB100;
  --foam:#F4F7FB; --white:#fff; --mist:#D7DDE3; --muted:#5B6472;
  --radius:14px;
}
*{box-sizing:border-box}
body{margin:0;background:var(--foam);color:var(--ink);
  font:15px/1.5 ui-sans-serif,system-ui,"Inter",sans-serif}
header{background:var(--abyss);color:#fff;padding:18px 22px;display:flex;
  align-items:center;gap:14px;flex-wrap:wrap}
header h1{font-size:18px;margin:0;letter-spacing:.02em}
header .tag{color:#B9C2E8;font-size:13px}
.chip{margin-left:auto;display:inline-flex;align-items:center;gap:8px;
  border:1px solid rgba(255,255,255,.25);border-radius:999px;padding:5px 12px;font-size:13px}
.dot{width:9px;height:9px;border-radius:50%;background:var(--mint)}
.dot.off{background:var(--coral)}
main{max-width:960px;margin:22px auto;padding:0 18px;display:grid;gap:18px}
.card{background:#fff;border:1px solid var(--mist);border-radius:var(--radius);padding:18px}
.card h2{margin:0 0 12px;font-size:15px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)}
.zones{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:14px}
.zone{border:1px solid var(--mist);background:#fff;border-radius:999px;padding:7px 14px;
  cursor:pointer;font:inherit}
.zone[aria-selected=true]{border-color:var(--current);
  box-shadow:inset 0 0 0 1px var(--current);font-weight:600}
.zone:focus-visible{outline:3px solid var(--trench);outline-offset:2px}
.row{display:flex;justify-content:space-between;gap:16px;padding:9px 0;
  border-bottom:1px dashed var(--mist)}
.row:last-child{border-bottom:0}
.k{color:var(--muted)}
.stops{display:flex;gap:8px;flex-wrap:wrap}
.stop{display:inline-flex;align-items:center;gap:6px;font-variant-numeric:tabular-nums}
.sw{width:15px;height:15px;border-radius:4px;border:1px solid rgba(0,0,0,.2)}
button.act{font:inherit;border-radius:10px;padding:10px 16px;cursor:pointer;
  border:1px solid var(--mist);background:#fff}
button.act.primary{background:var(--current);border-color:var(--current);color:#00252e;font-weight:600}
button.act.risk{border-color:var(--amber)}
button.act:focus-visible{outline:3px solid var(--trench);outline-offset:2px}
.actions{display:flex;gap:10px;flex-wrap:wrap;margin-top:6px}
.ladder{display:flex;gap:6px;flex-wrap:wrap;margin:10px 0 0}
.step{font-size:12px;border:1px solid var(--mist);border-radius:999px;padding:3px 10px;color:var(--muted)}
.step.on{border-color:var(--current);color:var(--ink);font-weight:600}
.step.bad{border-color:var(--trench);color:var(--trench);font-weight:600}
.status{margin-top:12px;padding:12px 14px;border-radius:10px;background:var(--foam);
  border:1px solid var(--mist)}
.status b{font-variant:all-small-caps;letter-spacing:.08em}
.status.verified{border-color:var(--mint)}
.status.mismatch,.status.failed{border-color:var(--trench)}
.hint{color:var(--muted);font-size:13px}
textarea{width:100%;min-height:120px;font:13px ui-monospace,monospace;
  border:1px solid var(--mist);border-radius:10px;padding:10px}
dialog{border:1px solid var(--mist);border-radius:var(--radius);padding:20px;max-width:420px}
dialog::backdrop{background:rgba(7,16,31,.45)}
@media (prefers-reduced-motion: no-preference){.pulse{animation:p 1.2s ease-in-out infinite}}
@keyframes p{50%{opacity:.55}}
@media (prefers-color-scheme: dark){
  body{background:var(--ink);color:var(--foam)}
  .card,.zone,button.act{background:#0d1a2b;border-color:#1e3350;color:var(--foam)}
  .status{background:#0d1a2b;border-color:#1e3350}
  textarea{background:#0d1a2b;color:var(--foam);border-color:#1e3350}
}
</style></head><body>
<header>
  <h1>CLEVERTUNA</h1><span class="tag">Read the current.</span>
  <span class="chip"><span class="dot" id="dot"></span><span id="conn">connecting…</span></span>
</header>
<main>
  <section class="card">
    <h2>Lighting</h2>
    <div class="zones" id="zones" role="tablist"></div>
    <div id="detail"><p class="hint">Reading the keyboard…</p></div>
  </section>

  <section class="card">
    <h2>Apply a scheme</h2>
    <p class="hint">Paste a scheme file. It is validated, sent, then read back and compared.</p>
    <textarea id="scheme" spellcheck="false" aria-label="Scheme JSON"></textarea>
    <div class="actions">
      <button class="act primary" id="apply">Send and verify</button>
      <button class="act" id="reload">Re-read keyboard</button>
      <button class="act risk" id="backup">Back up everything</button>
    </div>
    <div class="ladder" id="ladder"></div>
    <div class="status" id="status" role="status" aria-live="polite">
      <b>ready</b> — nothing sent yet
    </div>
  </section>
</main>

<dialog id="confirm">
  <h3 style="margin-top:0">Send this scheme?</h3>
  <p>This rewrites the keyboard's lighting. Other settings are left untouched.</p>
  <div class="actions">
    <button class="act primary" id="yes">Send</button>
    <button class="act" id="no">Cancel</button>
  </div>
</dialog>

<script>
const STAGES=["validated","sent","acknowledged","read_back","verified"];
let state=null, zone=0;
const $=id=>document.getElementById(id);

function ladder(stage){
  const bad = stage==="mismatch"||stage==="failed";
  $("ladder").innerHTML = STAGES.map(s=>{
    const idx=STAGES.indexOf(stage), on=idx>=0 && STAGES.indexOf(s)<=idx;
    return `<span class="step ${on?"on":""}">${s.replace("_"," ")}</span>`;
  }).join("") + (bad?`<span class="step bad">${stage}</span>`:"");
}
function status(stage,msg){
  const el=$("status"); el.className="status "+stage;
  el.innerHTML=`<b>${stage.replace("_"," ")}</b> — ${msg}`;
  ladder(stage);
}
function hex(c){const h=n=>(n||0).toString(16).padStart(2,"0").toUpperCase();
  return "#"+h(c.red)+h(c.green)+h(c.blue);}
const LABEL={keyboard:"keyboard",touchpad:"touchpad",leftSlider:"left slider",rightSlider:"right slider"};
const EFFECTS=["solidColor","breathing","colorCycle","colorWave","aurora"];
const ENAME={solidColor:"Solid colour",breathing:"Breathing",colorCycle:"Colour cycle",
             colorWave:"Colour wave",aurora:"Aurora"};

function render(){
  const bl=state&&state.backlight; const keys=bl?Object.keys(LABEL).filter(k=>bl[k]):[];
  $("zones").innerHTML=keys.map((k,i)=>
    `<button class="zone" role="tab" aria-selected="${i===zone}" data-i="${i}">${LABEL[k]}</button>`).join("");
  [...document.querySelectorAll(".zone")].forEach(b=>
    b.onclick=()=>{zone=+b.dataset.i;render();});
  if(!keys.length){$("detail").innerHTML='<p class="hint">No lighting read yet.</p>';return;}
  const z=bl[keys[Math.min(zone,keys.length-1)]];
  const eff=EFFECTS.find(e=>z[e]);
  let rows=[`<div class="row"><span class="k">Effect</span><span>${ENAME[eff]||"—"}</span></div>`];
  const e=z[eff]||{};
  if(e.colorLinePicker&&e.colorLinePicker.markersArray){
    rows.push(`<div class="row"><span class="k">Stops</span><span class="stops">`+
      e.colorLinePicker.markersArray.map(m=>
        `<span class="stop"><span class="sw" style="background:${hex(m.color||{})}"></span>${hex(m.color||{})}</span>`
      ).join("")+`</span></div>`);
  }
  if(e.color) rows.push(`<div class="row"><span class="k">Colour</span><span class="stop">
      <span class="sw" style="background:${hex(e.color)}"></span>${hex(e.color)}</span></div>`);
  for(const [k,l] of [["direction","Direction"],["period","Period"],["length","Length"]])
    if(e[k]!==undefined) rows.push(`<div class="row"><span class="k">${l}</span><span>${e[k]}${k==="period"?" ms":k==="direction"?"°":""}</span></div>`);
  if(z.interactiveAnimation) rows.push(`<div class="row"><span class="k">Interactive</span><span>${z.interactiveAnimation.enable?"on":"off"}</span></div>`);
  if(z.transparency!==undefined) rows.push(`<div class="row"><span class="k">Transparency</span><span>${z.transparency}</span></div>`);
  $("detail").innerHTML=rows.join("");
}

async function load(){
  status("validated","reading the keyboard…");
  try{
    const r=await fetch("/api/state"); state=await r.json();
    $("dot").className="dot"+(state.connected?"":" off");
    $("conn").textContent=state.connected?`${(state.transport||"").toUpperCase()} · connected`:"no device";
    if(state.connected){
      status("read_back","read back from the device");
      if(!$("scheme").value.trim())
        $("scheme").value=JSON.stringify({clevertuna_backlight:1,backlight:state.backlight},null,2);
    } else status("failed",state.message||"no configuration interface found");
    render();
  }catch(e){status("failed",String(e));}
}
$("reload").onclick=load;
$("apply").onclick=()=>$("confirm").showModal();
$("no").onclick=()=>$("confirm").close();
$("yes").onclick=async()=>{
  $("confirm").close();
  let doc; try{doc=JSON.parse($("scheme").value);}catch(e){return status("failed","that is not valid JSON");}
  status("sent","sending…");
  try{
    const r=await fetch("/api/apply",{method:"POST",headers:{"Content-Type":"application/json"},
      body:JSON.stringify(doc)});
    const out=await r.json();
    status(out.stage,out.message||"");
    if(out.stage==="verified"||out.stage==="mismatch") load();
  }catch(e){status("failed",String(e));}
};
$("backup").onclick=async()=>{
  status("sent","reading every setting…");
  const r=await fetch("/api/backup",{method:"POST"}); const out=await r.json();
  status(out.stage,out.file?`saved ${out.bytes} bytes to ${out.file}`:(out.message||""));
};
load();
</script></body></html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_is_self_contained_and_offline() {
        // no external origins may appear in the shipped page
        for bad in ["http://", "https://", "//cdn", "fonts.googleapis"] {
            assert!(!PAGE.contains(bad), "page references {}", bad);
        }
    }

    #[test]
    fn page_covers_the_required_states() {
        for needed in ["validated", "sent", "acknowledged", "read_back", "verified", "mismatch"] {
            assert!(PAGE.contains(needed), "page never mentions {}", needed);
        }
    }

    #[test]
    fn page_has_a_confirmation_before_writing() {
        assert!(PAGE.contains("<dialog"));
        assert!(PAGE.contains("showModal"));
    }

    #[test]
    fn page_distinguishes_transports_without_relying_on_colour() {
        assert!(PAGE.contains("connected"));
        assert!(PAGE.contains("no device"));
        assert!(PAGE.contains("toUpperCase"));
    }

    #[test]
    fn page_respects_reduced_motion_and_dark_mode() {
        assert!(PAGE.contains("prefers-reduced-motion"));
        assert!(PAGE.contains("prefers-color-scheme"));
    }

    #[test]
    fn page_has_visible_focus_styles() {
        assert!(PAGE.contains(":focus-visible"));
    }
}
