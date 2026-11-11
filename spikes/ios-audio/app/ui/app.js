// Тестовая страница спайка: без сборщика, Tauri API из window.__TAURI__.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const state = { playing: false, durationMs: 0, positionMs: 0, dragging: false, mode: "gapless" };

const fmt = (ms) => {
  if (!ms && ms !== 0) return "—";
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

const STATE_RU = { Idle: "Остановлено", Loading: "Загрузка", Playing: "Играет", Paused: "Пауза", Ended: "Очередь доиграна" };

function setPlaying(playing) {
  state.playing = playing;
  $("toggle").textContent = playing ? "⏸" : "▶";
}

function renderPosition() {
  $("pos").textContent = fmt(state.positionMs);
  $("dur").textContent = fmt(state.durationMs);
  if (!state.dragging && state.durationMs > 0) {
    $("seek").value = Math.round((state.positionMs / state.durationMs) * 1000);
  }
}

listen("engine", ({ payload: e }) => {
  switch (e.type) {
    case "state":
      $("state").textContent = STATE_RU[e.state] ?? e.state;
      setPlaying(e.state === "Playing");
      break;
    case "position":
      state.positionMs = e.positionMs;
      if (e.durationMs) state.durationMs = e.durationMs;
      renderPosition();
      break;
    case "track":
      $("title").textContent = e.title || "Без названия";
      $("artist").textContent = e.artist || "—";
      $("album").textContent = `${e.album} · трек ${e.index + 1}${e.auto ? " · сам" : ""}`;
      state.durationMs = e.durationMs || 0;
      state.positionMs = 0;
      $("cover").style.backgroundImage = e.artwork ? `url("${e.artwork}")` : "";
      renderPosition();
      break;
    case "buffering":
      $("buffering").classList.toggle("hidden", !e.on);
      break;
    case "firstAudio":
      $("ttfa").textContent = `Звук через ${Math.round(e.latencyMs)} мс`;
      $("ttfa").classList.remove("hidden");
      break;
    case "info":
      $("info").textContent = e.info;
      break;
    case "error":
      $("serverStatus").textContent = `Ошибка: ${e.message}`;
      break;
  }
});

listen("selftest", ({ payload: r }) => {
  const li = document.createElement("li");
  if (r.error) {
    li.className = "fail";
    li.textContent = `Ошибка: ${r.error}`;
  } else if (r.done) {
    li.textContent = "Готово";
    $("selftest").disabled = false;
  } else {
    li.className = r.exact && r.worstShift <= 1 ? "pass" : "fail";
    li.textContent = r.summary;
  }
  $("results").appendChild(li);
});

$("toggle").onclick = () => invoke("transport", { action: "toggle" });
$("prev").onclick = () => invoke("transport", { action: "previous" });
$("next").onclick = () => invoke("transport", { action: "next" });
$("seek").addEventListener("input", () => (state.dragging = true));
$("seek").addEventListener("change", () => {
  state.dragging = false;
  if (state.durationMs > 0) invoke("transport", { action: "seek", value: ($("seek").value / 1000) * state.durationMs });
});
$("volume").addEventListener("input", () => invoke("transport", { action: "volume", value: $("volume").value / 100 }));

const applyMode = () => {
  const secs = Number($("xfade").value);
  $("xfadeValue").textContent = `${secs} с`;
  invoke("transport", state.mode === "gapless" ? { action: "gapless" } : { action: "crossfade", value: secs * 1000 });
};
document.querySelectorAll(".segmented button").forEach((b) => {
  b.onclick = () => {
    document.querySelectorAll(".segmented button").forEach((x) => {
      x.classList.toggle("active", x === b);
      x.setAttribute("aria-checked", String(x === b));
    });
    state.mode = b.dataset.mode;
    applyMode();
  };
});
$("xfade").addEventListener("change", applyMode);
$("xfade").addEventListener("input", () => ($("xfadeValue").textContent = `${$("xfade").value} с`));

function renderAlbums(manifest) {
  const list = $("albums");
  list.innerHTML = "";
  const all = [{ id: "long", title: "Длинная очередь для фона", tracks: { length: "все" } }, ...manifest.albums];
  for (const a of all) {
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.innerHTML = `<span class="name"></span><span class="count"></span>`;
    b.querySelector(".name").textContent = a.title;
    b.querySelector(".count").textContent = `${a.tracks.length} тр.`;
    b.onclick = async () => {
      try {
        const n = await invoke("load_album", { albumId: a.id, autoplay: true });
        $("serverStatus").textContent = `В очереди ${n} треков`;
      } catch (e) {
        $("serverStatus").textContent = `Ошибка: ${e}`;
      }
    };
    li.appendChild(b);
    list.appendChild(li);
  }
}

async function connect(url) {
  $("serverStatus").textContent = "Подключаюсь…";
  try {
    const m = url ? await invoke("set_server", { url }) : await invoke("get_manifest");
    renderAlbums(m);
    $("serverStatus").textContent = `Наборов: ${m.albums.length}`;
  } catch (e) {
    $("serverStatus").textContent = `Сервер недоступен: ${e}`;
  }
}

$("connect").onclick = () => connect($("server").value);
$("selftest").onclick = async () => {
  $("results").innerHTML = "";
  $("selftest").disabled = true;
  try {
    await invoke("run_selftest");
  } catch (e) {
    $("selftest").disabled = false;
    $("results").innerHTML = `<li class="fail">${e}</li>`;
  }
};
$("npRefresh").onclick = async () => ($("nowplaying").textContent = `«Сейчас играет»: ${await invoke("now_playing")}`);
document.querySelectorAll("[data-sim]").forEach((b) => {
  b.onclick = async () => {
    try {
      await invoke("simulate", { kind: b.dataset.sim });
    } catch (e) {
      $("serverStatus").textContent = String(e);
    }
  };
});

async function refreshStats() {
  try {
    const s = await invoke("get_stats");
    $("s-cpu").textContent = `${s.cpu_percent.toFixed(1)} %`;
    $("s-mem").textContent = s.footprint_mb ? `${s.footprint_mb.toFixed(1)} МБ` : "—";
    $("s-load").textContent = `${(s.callback_load_avg * 100).toFixed(2)} % (макс. ${(s.callback_load_max * 100).toFixed(2)} %)`;
    $("s-under").textContent = String(s.underruns);
    $("s-dev").textContent = `${s.device_rate} Гц, буфер ${s.buffer_frames}`;
    $("s-up").textContent = `${Math.round(s.uptime_s)} с`;
    const lines = await invoke("get_log");
    $("log").textContent = lines.slice(-60).join("\n");
  } catch (_) {
    // окно скрыто или движок занят — следующий опрос
  }
}

(async () => {
  const c = await invoke("get_config");
  $("platform").textContent = `${c.platform} · ${c.arch} · вывод ${c.deviceRate} Гц`;
  $("server").value = c.server;
  connect();
  setInterval(refreshStats, 2000);
  refreshStats();
})();
