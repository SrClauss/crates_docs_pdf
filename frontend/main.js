// Tauri APIs
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// UI Elements
const crateNameInput = document.getElementById("crate-name");
const searchBtn = document.getElementById("search-btn");
const translateCheckbox = document.getElementById("translate-checkbox");
const statusLabel = document.getElementById("status-label");
const timerLabel = document.getElementById("timer-label");
const progressBar = document.getElementById("progress-bar");
const resultsBody = document.getElementById("results-body");
const closeBtn = document.getElementById("close-btn");
const minBtn = document.getElementById("min-btn");

// State
let selectedCrate = null;
let timerInterval = null;
let startTime = null;
let currentProgress = 0.0;
let isBusy = false;

// 1. Close Button Handler
closeBtn.addEventListener("click", () => {
  try {
    if (window.__TAURI__ && window.__TAURI__.webviewWindow) {
      window.__TAURI__.webviewWindow.getCurrentWebviewWindow().close();
    } else if (window.__TAURI__ && window.__TAURI__.window) {
      window.__TAURI__.window.getCurrentWindow().close();
    } else {
      window.close();
    }
  } catch (e) {
    console.error("Failed to close window via Tauri, using fallback:", e);
    window.close();
  }
});

// Minimize Button Handler
minBtn.addEventListener("click", () => {
  try {
    if (window.__TAURI__ && window.__TAURI__.webviewWindow) {
      window.__TAURI__.webviewWindow.getCurrentWebviewWindow().minimize();
    } else if (window.__TAURI__ && window.__TAURI__.window) {
      window.__TAURI__.window.getCurrentWindow().minimize();
    }
  } catch (e) {
    console.error("Failed to minimize window:", e);
  }
});

// Title Bar Window Dragging
const titleBar = document.querySelector(".xp-titlebar");
titleBar.addEventListener("mousedown", (e) => {
  // Do not drag if clicking buttons inside titlebar
  if (e.target.closest(".xp-titlebar-buttons")) {
    return;
  }
  if (e.buttons === 1) { // Left click
    try {
      if (window.__TAURI__ && window.__TAURI__.webviewWindow) {
        window.__TAURI__.webviewWindow.getCurrentWebviewWindow().startDragging();
      } else if (window.__TAURI__ && window.__TAURI__.window) {
        window.__TAURI__.window.getCurrentWindow().startDragging();
      }
    } catch (err) {
      console.error("Failed to drag window:", err);
    }
  }
});

// 2. Ticking Timer
function startTimer() {
  stopTimer();
  startTime = Date.now();
  timerInterval = setInterval(() => {
    const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
    timerLabel.textContent = `(${elapsed}s)`;
  }, 100);
}

function stopTimer() {
  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
}

// 3. Update Progress Bar Blocks
function updateProgressBar(progress) {
  currentProgress = progress;
  const barWidth = progressBar.clientWidth;
  
  // XP style block: 8px width + 2px spacing = 10px total
  const maxBlocks = Math.floor(barWidth / 10);
  const activeBlocks = Math.round(progress * maxBlocks);
  
  progressBar.innerHTML = "";
  for (let i = 0; i < activeBlocks; i++) {
    const block = document.createElement("div");
    block.className = "xp-progress-block";
    progressBar.appendChild(block);
  }
}

// Handle window resizing to keep the progress blocks accurate
window.addEventListener("resize", () => {
  updateProgressBar(currentProgress);
});

// 4. Set Busy State
function setBusy(busy) {
  isBusy = busy;
  crateNameInput.disabled = busy;
  searchBtn.disabled = busy || crateNameInput.value.trim() === "";
  translateCheckbox.disabled = busy;
  
  // Disable all rows in table
  const rows = resultsBody.querySelectorAll("tr");
  rows.forEach(r => {
    if (busy) {
      r.style.opacity = "0.6";
      r.style.pointerEvents = "none";
    } else {
      r.style.opacity = "1";
      r.style.pointerEvents = "auto";
    }
  });
}

// 5. Input key listener
crateNameInput.addEventListener("input", () => {
  if (!isBusy) {
    searchBtn.disabled = crateNameInput.value.trim() === "";
  }
});

crateNameInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !searchBtn.disabled) {
    performSearch();
  }
});

searchBtn.addEventListener("click", performSearch);

// 6. Search function
async function performSearch() {
  const query = crateNameInput.value.trim();
  if (!query) return;

  setBusy(true);
  statusLabel.textContent = `Buscando '${query}' no crates.io...`;
  startTimer();
  updateProgressBar(0.0);
  resultsBody.innerHTML = `<tr><td colspan="2" class="xp-listview-empty">Buscando crates, por favor aguarde...</td></tr>`;

  try {
    const results = await invoke("search_crates", { query });
    stopTimer();
    setBusy(false);
    timerLabel.textContent = "(0.0s)";
    
    if (results.length === 0) {
      statusLabel.textContent = "Nenhuma crate encontrada.";
      resultsBody.innerHTML = `<tr><td colspan="2" class="xp-listview-empty">Nenhum resultado encontrado para '${query}'.</td></tr>`;
      return;
    }

    statusLabel.textContent = "Busca concluída. Escolha uma crate para gerar o PDF!";
    resultsBody.innerHTML = "";
    
    results.forEach(c => {
      const row = document.createElement("tr");
      
      const nameCell = document.createElement("td");
      nameCell.className = "col-name";
      nameCell.textContent = c.name;
      
      const descCell = document.createElement("td");
      descCell.className = "col-desc";
      descCell.textContent = c.desc;
      
      row.appendChild(nameCell);
      row.appendChild(descCell);
      
      row.addEventListener("click", () => {
        // Highlight row
        const selectedRow = resultsBody.querySelector("tr.selected");
        if (selectedRow) selectedRow.classList.remove("selected");
        row.classList.add("selected");
        
        generateCratePdf(c);
      });
      
      resultsBody.appendChild(row);
    });
  } catch (err) {
    stopTimer();
    setBusy(false);
    statusLabel.textContent = `Erro na busca: ${err}`;
    resultsBody.innerHTML = `<tr><td colspan="2" class="xp-listview-empty" style="color: #ff0000;">Erro na busca: ${err}</td></tr>`;
  }
}

// 7. PDF Generation function
async function generateCratePdf(crate) {
  if (isBusy) return;
  
  setBusy(true);
  startTimer();
  updateProgressBar(0.0);
  statusLabel.textContent = `Iniciando manual PDF para '${crate.name}'...`;
  
  const translate = translateCheckbox.checked;

  try {
    const filename = await invoke("generate_pdf", {
      name: crate.name,
      href: crate.href,
      translate: translate
    });
    
    stopTimer();
    setBusy(false);
    updateProgressBar(1.0);
    statusLabel.textContent = `Sucesso! Manual salvo em: ${filename}`;
  } catch (err) {
    stopTimer();
    setBusy(false);
    updateProgressBar(0.0);
    statusLabel.textContent = `Erro na geração: ${err}`;
  }
}

// 8. Listen to progress events from Rust backend
listen("progress", (event) => {
  const payload = event.payload;
  statusLabel.textContent = payload.status;
  updateProgressBar(payload.progress);
});
