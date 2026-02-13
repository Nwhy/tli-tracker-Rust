use axum::{
    extract::Json,
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{DropItem, Session};
use crate::{log_parser, storage};

pub async fn serve(addr: String) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .route("/overlay", get(overlay))
        .route("/api/sessions", get(api_sessions))
        .route("/api/start", post(api_start))
        .route("/api/end", post(api_end))
        .route("/api/drop", post(api_drop))
        .route("/api/game-path", get(api_game_path))
        .route("/api/loot", get(api_loot))
        .route("/api/inventory", get(api_inventory));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Web UI running on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn overlay() -> Html<&'static str> {
    Html(OVERLAY_HTML)
}

async fn api_sessions() -> Result<Json<Vec<Session>>, (StatusCode, String)> {
    let sessions = storage::load_sessions().map_err(io_err)?;
    Ok(Json(sessions))
}

#[derive(Deserialize)]
struct StartReq {
    map: String,
    notes: Option<String>,
}

async fn api_start(Json(payload): Json<StartReq>) -> Result<Json<Session>, (StatusCode, String)> {
    let mut sessions = storage::load_sessions().map_err(io_err)?;
    let session = Session {
        id: Uuid::new_v4().to_string(),
        map: payload.map,
        notes: payload.notes,
        start_time: Utc::now(),
        end_time: None,
        drops: Vec::new(),
    };
    sessions.push(session.clone());
    storage::save_sessions(&sessions).map_err(io_err)?;
    Ok(Json(session))
}

#[derive(Deserialize)]
struct EndReq {
    session: Option<String>,
}

async fn api_end(Json(payload): Json<EndReq>) -> Result<Json<Session>, (StatusCode, String)> {
    let mut sessions = storage::load_sessions().map_err(io_err)?;
    let target_id = resolve_session_id(&sessions, payload.session).map_err(|e| api_err(&e.to_string()))?;
    let session = sessions
        .iter_mut()
        .find(|s| s.id == target_id)
        .ok_or_else(|| api_err("Session not found"))?;

    if session.end_time.is_none() {
        session.end_time = Some(Utc::now());
    }
    let result = session.clone();
    storage::save_sessions(&sessions).map_err(io_err)?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct DropReq {
    name: String,
    quantity: Option<u32>,
    value: f64,
    session: Option<String>,
}

async fn api_drop(Json(payload): Json<DropReq>) -> Result<Json<Session>, (StatusCode, String)> {
    let mut sessions = storage::load_sessions().map_err(io_err)?;
    let target_id = resolve_session_id(&sessions, payload.session).map_err(|e| api_err(&e.to_string()))?;

    let session = sessions
        .iter_mut()
        .find(|s| s.id == target_id)
        .ok_or_else(|| api_err("Session not found"))?;

    let drop = DropItem {
        name: payload.name,
        quantity: payload.quantity.unwrap_or(1),
        value: payload.value,
    };
    session.drops.push(drop);
    let result = session.clone();

    storage::save_sessions(&sessions).map_err(io_err)?;
    Ok(Json(result))
}

fn resolve_session_id(
    sessions: &[Session],
    requested: Option<String>,
) -> anyhow::Result<String> {
    if let Some(id) = requested {
        return Ok(id);
    }

    let active = sessions.iter().find(|s| s.is_active());
    if let Some(session) = active {
        return Ok(session.id.clone());
    }

    Err(anyhow::anyhow!(
        "No active session found. Specify a session id."
    ))
}

fn io_err(err: std::io::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn api_err(message: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message.to_string())
}

async fn api_game_path() -> Json<serde_json::Value> {
    let game_path = storage::detect_game_path();
    let log_path = storage::detect_game_log();
    Json(serde_json::json!({
        "game_path": game_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "log_path": log_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "log_found": log_path.is_some(),
    }))
}

async fn api_loot() -> Result<Json<log_parser::LootSummary>, (StatusCode, String)> {
    let log_path = storage::detect_game_log()
        .ok_or_else(|| api_err("UE_game.log not found"))?;
    let summary = log_parser::parse_loot_from_log(&log_path).map_err(io_err)?;
    Ok(Json(summary))
}

async fn api_inventory() -> Result<Json<Vec<log_parser::BagEvent>>, (StatusCode, String)> {
    let log_path = storage::detect_game_log()
        .ok_or_else(|| api_err("UE_game.log not found"))?;
    let inv = log_parser::parse_inventory_from_log(&log_path).map_err(io_err)?;
    Ok(Json(inv))
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>TLI Tracker</title>
  <style>
    :root {
      --bg: #070a12;
      --panel: #0d1220;
      --panel-2: #0f172a;
      --border: #1f2a44;
      --accent: #3b82f6;
      --accent-2: #22d3ee;
      --text: #e5e7eb;
      --muted: #9ca3af;
      --success: #22c55e;
      --danger: #ef4444;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0; font-family: "Inter", system-ui, sans-serif;
      background: radial-gradient(1200px 800px at 20% -20%, #172554 0%, #0b1020 40%, var(--bg) 100%);
      color: var(--text);
    }
    header {
      padding: 24px 28px;
      border-bottom: 1px solid var(--border);
      display: flex; align-items: center; justify-content: space-between;
      background: rgba(7,10,18,0.9); backdrop-filter: blur(6px);
      position: sticky; top: 0; z-index: 10;
    }
    header h1 { margin: 0; font-size: 20px; letter-spacing: 0.5px; }
    header .badge { padding: 6px 10px; border: 1px solid var(--border); border-radius: 999px; color: var(--muted); font-size: 12px; }
    main { padding: 24px 28px 40px; display: grid; gap: 16px; grid-template-columns: 360px 1fr; }
    .panel { background: var(--panel); border: 1px solid var(--border); border-radius: 14px; padding: 16px; }
    .panel h3 { margin: 0 0 12px; font-size: 14px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted); }
    .grid { display: grid; gap: 12px; }
    .stats { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
    .stat { background: var(--panel-2); border: 1px solid var(--border); border-radius: 12px; padding: 12px; }
    .stat .label { color: var(--muted); font-size: 12px; }
    .stat .value { font-size: 22px; margin-top: 6px; font-weight: 600; }
    .row { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
    label { display:block; margin: 6px 0 4px; color: var(--muted); font-size: 12px; }
    input {
      width: 100%; padding: 10px; border-radius: 10px;
      border: 1px solid var(--border); background: #0b1424; color: var(--text);
    }
    button {
      margin-top: 10px; padding: 10px 14px; background: var(--accent);
      color: white; border: none; border-radius: 10px; cursor: pointer; font-weight: 600;
    }
    .btn-secondary { background: #1f2937; color: var(--text); border: 1px solid var(--border); }
    .btn-danger { background: var(--danger); }
    .session-list { display: grid; gap: 10px; max-height: 520px; overflow: auto; }
    .session-item { background: #0b1222; border: 1px solid var(--border); border-radius: 12px; padding: 12px; }
    .session-title { display:flex; align-items:center; justify-content:space-between; font-size: 14px; }
    .pill { padding: 4px 8px; border-radius: 999px; font-size: 11px; border: 1px solid var(--border); color: var(--muted); }
    .pill.active { color: #dbeafe; border-color: #1d4ed8; background: rgba(59,130,246,0.15); }
    .muted { color: var(--muted); font-size: 12px; }
    pre { white-space: pre-wrap; margin: 0; }
    @media (max-width: 1100px) { main { grid-template-columns: 1fr; } .stats { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  </style>
</head>
<body>
  <header>
    <h1>TLI Tracker</h1>
    <div style="display:flex;align-items:center;gap:10px;">
      <div id="log-status" class="badge" title="Detecting UE_game.log...">⏳ Log: detecting...</div>
      <div class="badge">CachyOS • Local UI</div>
    </div>
  </header>

  <main>
    <section class="panel grid">
      <h3>Control</h3>
      <div class="panel" style="padding:12px;">
        <div class="row">
          <div>
            <label>Map</label><input id="map" placeholder="Map name" />
          </div>
          <div>
            <label>Notes</label><input id="notes" placeholder="Optional notes" />
          </div>
        </div>
        <button onclick="startSession()">Start Session</button>
      </div>

      <div class="panel" style="padding:12px;">
        <div class="row">
          <div>
            <label>Drop Name</label><input id="drop-name" placeholder="Item name" />
          </div>
          <div>
            <label>Value</label><input id="drop-value" type="number" step="0.01" value="0" />
          </div>
        </div>
        <div class="row">
          <div>
            <label>Quantity</label><input id="drop-qty" type="number" value="1" />
          </div>
          <div>
            <label>Session ID (optional)</label><input id="drop-session" placeholder="Active by default" />
          </div>
        </div>
        <button onclick="addDrop()">Add Drop</button>
        <button class="btn-danger" onclick="endSession()">End Active Session</button>
      </div>
    </section>

    <section class="panel">
      <h3>Overview</h3>
      <div class="stats">
        <div class="stat"><div class="label">Active Map</div><div class="value" id="stat-map">-</div></div>
        <div class="stat"><div class="label">Drops</div><div class="value" id="stat-drops">0</div></div>
        <div class="stat"><div class="label">Total Value</div><div class="value" id="stat-total">0</div></div>
        <div class="stat"><div class="label">Profit / Min</div><div class="value" id="stat-ppm">0</div></div>
      </div>

      <div style="height:12px"></div>

      <div class="panel" style="padding:12px;">
        <div class="session-title" style="margin-bottom:10px;">
          <strong>Sessions</strong>
          <span class="muted">auto-refresh</span>
        </div>
        <div id="session-list" class="session-list">Loading...</div>
      </div>

      <div id="loot-panel" class="panel" style="padding:12px; display:none;">
        <div class="session-title" style="margin-bottom:10px;">
          <strong>Game Loot (UE_game.log)</strong>
          <span class="muted" id="loot-events">0 events</span>
        </div>
        <div id="loot-list" class="session-list" style="max-height:320px;">Waiting for log data...</div>
      </div>
    </section>
  </main>

<script>
function totalValue(drops){
  return drops.reduce((acc, d) => acc + (d.value * d.quantity), 0);
}
function minutesSince(start, end){
  if(!start || !end) return null;
  const s = new Date(start);
  const e = new Date(end);
  return (e - s) / 60000;
}
async function refreshSessions(){
  const res = await fetch('/api/sessions');
  const data = await res.json();
  const active = data.find(s => !s.end_time);

  if(active){
    document.getElementById('stat-map').textContent = active.map;
    document.getElementById('stat-drops').textContent = active.drops.length;
    const total = totalValue(active.drops);
    document.getElementById('stat-total').textContent = total.toFixed(2);
    const mins = minutesSince(active.start_time, new Date().toISOString());
    const ppm = mins && mins > 0 ? total / mins : 0;
    document.getElementById('stat-ppm').textContent = ppm.toFixed(2);
  } else {
    document.getElementById('stat-map').textContent = '-';
    document.getElementById('stat-drops').textContent = '0';
    document.getElementById('stat-total').textContent = '0';
    document.getElementById('stat-ppm').textContent = '0';
  }

  const list = document.getElementById('session-list');
  list.innerHTML = '';
  if(data.length === 0){
    list.textContent = 'No sessions yet.';
    return;
  }
  data.slice().reverse().forEach(s => {
    const el = document.createElement('div');
    el.className = 'session-item';
    const status = s.end_time ? 'ended' : 'active';
    const pillClass = s.end_time ? 'pill' : 'pill active';
    const total = totalValue(s.drops).toFixed(2);
    el.innerHTML = `
      <div class="session-title">
        <div><strong>${s.map}</strong> <span class="muted">${s.id.slice(0,8)}</span></div>
        <span class="${pillClass}">${status}</span>
      </div>
      <div class="muted">Drops: ${s.drops.length} • Total: ${total}</div>
    `;
    list.appendChild(el);
  });
}
async function startSession(){
  const map = document.getElementById('map').value.trim();
  const notes = document.getElementById('notes').value.trim();
  if(!map){ alert('Map required'); return; }
  await fetch('/api/start', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({ map, notes: notes || null }) });
  await refreshSessions();
}
async function addDrop(){
  const name = document.getElementById('drop-name').value.trim();
  const quantity = parseInt(document.getElementById('drop-qty').value, 10);
  const value = parseFloat(document.getElementById('drop-value').value);
  const session = document.getElementById('drop-session').value.trim();
  if(!name){ alert('Name required'); return; }
  await fetch('/api/drop', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({ name, quantity, value, session: session || null }) });
  await refreshSessions();
}
async function endSession(){
  await fetch('/api/end', { method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({}) });
  await refreshSessions();
}
refreshSessions();
setInterval(refreshSessions, 3000);
(async function checkGameLog(){
  try {
    const res = await fetch('/api/game-path');
    const data = await res.json();
    const el = document.getElementById('log-status');
    if(data.log_found){
      el.textContent = '✅ Log: found';
      el.title = data.log_path;
      el.style.borderColor = '#166534';
      el.style.color = '#86efac';
      document.getElementById('loot-panel').style.display = '';
      refreshLoot();
      setInterval(refreshLoot, 5000);
    } else {
      el.textContent = '❌ Log: not found';
      el.title = 'UE_game.log not found. Make sure Torchlight Infinite is installed via Steam and logging is enabled in game settings.';
      el.style.borderColor = '#991b1b';
      el.style.color = '#fca5a5';
    }
  } catch(e) {
    const el = document.getElementById('log-status');
    el.textContent = '⚠ Log: error';
    el.title = 'Failed to check game log path';
  }
})();
async function refreshLoot(){
  try {
    const res = await fetch('/api/loot');
    if(!res.ok) return;
    const data = await res.json();
    document.getElementById('loot-events').textContent = data.total_events + ' events';
    const list = document.getElementById('loot-list');
    list.innerHTML = '';
    if(!data.items || data.items.length === 0){
      list.textContent = 'No loot detected yet. Sort your inventory in-game to sync, then pick up items.';
      return;
    }
    data.items.forEach(item => {
      const el = document.createElement('div');
      el.className = 'session-item';
      const sign = item.delta > 0 ? '+' : '';
      const color = item.delta > 0 ? '#86efac' : '#fca5a5';
      el.innerHTML = `
        <div class="session-title">
          <div><strong>${item.item_name}</strong> <span class="muted">${item.config_base_id}</span></div>
          <span style="color:${color};font-weight:600;">${sign}${item.delta}</span>
        </div>
        <div class="muted">Current stack: ${item.current}</div>
      `;
      list.appendChild(el);
    });
  } catch(e) { /* ignore */ }
}
</script>
</body>
</html>
"#;

const OVERLAY_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>TLI Overlay</title>
  <style>
    :root {
      --glass: rgba(9, 12, 20, 0.65);
      --border: rgba(148, 163, 184, 0.25);
      --text: #e5e7eb;
      --muted: #9ca3af;
      --accent: #38bdf8;
    }
    body { margin: 0; font-family: "Inter", system-ui, sans-serif; background: transparent; color: var(--text); }
    .overlay {
      padding: 10px 14px;
      background: var(--glass);
      border: 1px solid var(--border);
      border-radius: 12px;
      backdrop-filter: blur(6px);
      width: fit-content;
      box-shadow: 0 10px 30px rgba(0,0,0,0.35);
    }
    .row { display:flex; gap: 10px; align-items: stretch; }
    .tile { background: rgba(15, 23, 42, 0.65); border: 1px solid var(--border); border-radius: 10px; padding: 8px 10px; min-width: 120px; }
    .title { font-size: 11px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.08em; }
    .value { font-size: 20px; font-weight: 600; margin-top: 6px; }
    .accent { color: var(--accent); }
  </style>
</head>
<body>
  <div class="overlay">
    <div class="row">
      <div class="tile"><div class="title">Map</div><div class="value" id="map">-</div></div>
      <div class="tile"><div class="title">Drops</div><div class="value" id="drops">0</div></div>
      <div class="tile"><div class="title">Total</div><div class="value accent" id="total">0</div></div>
      <div class="tile"><div class="title">Status</div><div class="value" id="status">idle</div></div>
    </div>
  </div>

<script>
function totalValue(drops){
  return drops.reduce((acc, d) => acc + (d.value * d.quantity), 0);
}
async function refresh(){
  const res = await fetch('/api/sessions');
  const data = await res.json();
  const active = data.find(s => !s.end_time);
  if(!active){
    document.getElementById('map').textContent = '-';
    document.getElementById('drops').textContent = '0';
    document.getElementById('total').textContent = '0';
    document.getElementById('status').textContent = 'idle';
    return;
  }
  document.getElementById('map').textContent = active.map;
  document.getElementById('drops').textContent = active.drops.length;
  document.getElementById('total').textContent = totalValue(active.drops).toFixed(2);
  document.getElementById('status').textContent = 'active';
}
refresh();
setInterval(refresh, 1500);
</script>
</body>
</html>
"#;
