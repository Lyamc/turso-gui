pub const CSS: &str = r#"
:root {
  --bg: #0d0d0d;
  --bg-raised: #1a1a1a;
  --bg-header: #333333;
  --bg-cell: #1a1a1a;
  --bg-cell-alt: #121212;
  --bg-hover: #242424;
  --selected: #1a66cc;
  --accent: #0080ff;
  --text: #ffffff;
  --dim: #999999;
  --error: #ff6060;
  --success: #50dc50;
  --info: #409cff;
  --border: #2a2a2a;
  --input: #111111;
  font-family: "Segoe UI", "Inter", system-ui, sans-serif;
  color: var(--text);
  background: var(--bg);
  font-size: 13px;
}

* { box-sizing: border-box; }
html, body, #main { margin: 0; height: 100%; background: var(--bg); color: var(--text); }
button, input, select, textarea { font: inherit; color: inherit; }
button { cursor: pointer; }
button:disabled { opacity: 0.45; cursor: default; }

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

.toolbar, .tabs, .status, .browse-bar, .sql-bar, .pager {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-raised);
  flex-shrink: 0;
}
.status { border-top: 1px solid var(--border); border-bottom: none; min-height: 28px; }
.toolbar { justify-content: space-between; }
.toolbar-left, .status { gap: 8px; }
.path { color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.btn {
  background: var(--bg-header);
  border: 1px solid var(--border);
  color: var(--text);
  padding: 5px 12px;
  border-radius: 4px;
}
.btn:hover:not(:disabled) { background: var(--selected); }
.btn.primary { background: var(--accent); border-color: var(--accent); }
.btn.primary:hover:not(:disabled) { filter: brightness(1.1); }

.tabs { padding: 0; gap: 0; }
.tab {
  background: #333;
  color: var(--dim);
  border: none;
  border-right: 1px solid var(--border);
  min-width: 96px;
  height: 32px;
  padding: 0 16px;
}
.tab.active { background: var(--selected); color: #fff; font-weight: 600; }
.tab:hover:not(.active) { background: #3d3d3d; }

.content { flex: 1; min-height: 0; display: flex; overflow: hidden; }

.connect {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
}
.connect h1 { font-size: 28px; font-weight: 700; margin: 0 0 8px; }
.connect-row { display: flex; gap: 8px; }
.connect input {
  width: 420px;
  max-width: 80vw;
  background: var(--input);
  border: 1px solid var(--border);
  padding: 8px 10px;
  border-radius: 4px;
}

.sidebar {
  width: 220px;
  flex-shrink: 0;
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  background: var(--bg-raised);
  overflow: hidden;
}
.sidebar h2, .panel h2 { font-size: 13px; margin: 0; padding: 10px 12px 6px; }
.list { overflow: auto; flex: 1; }
.list-item {
  display: block;
  width: 100%;
  text-align: left;
  background: transparent;
  border: none;
  color: var(--text);
  padding: 6px 12px;
}
.list-item:hover { background: var(--bg-hover); }
.list-item.active { background: var(--selected); }
.muted { color: var(--dim); padding: 8px 12px; }

.panel { flex: 1; min-width: 0; overflow: auto; padding: 8px 16px 16px; }
.schema {
  white-space: pre-wrap;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  color: var(--dim);
  background: var(--bg-cell);
  border: 1px solid var(--border);
  padding: 10px;
  border-radius: 4px;
}

.cols { width: 100%; border-collapse: collapse; }
.cols th, .cols td { text-align: left; padding: 6px 10px; border-bottom: 1px solid var(--border); }
.cols th { color: var(--dim); font-weight: 600; }

.split { flex: 1; min-width: 0; display: flex; overflow: hidden; }
.main-col { flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden; }
.editor-pane {
  width: 280px;
  flex-shrink: 0;
  border-left: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  background: var(--bg-raised);
  padding: 8px;
}
.editor-pane h2 { margin: 0 0 6px; font-size: 13px; }
.cell-editor, .sql-input {
  flex: 1;
  width: 100%;
  background: var(--input);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 8px;
  resize: none;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
}

.browse-bar select, .pager select, .pager input[type="number"] {
  background: var(--input);
  border: 1px solid var(--border);
  padding: 4px 8px;
  border-radius: 4px;
}
.pager { border-top: 1px solid var(--border); border-bottom: none; justify-content: flex-start; color: var(--dim); }
.pager label { display: flex; align-items: center; gap: 6px; }

.sql-bar { align-items: stretch; }
.sql-input { min-height: 88px; height: 120px; }
.sql-exec { align-self: flex-start; }

.grid-wrap { flex: 1; min-height: 0; overflow: auto; }
.grid { border-collapse: collapse; min-width: 100%; }
.grid th, .grid td {
  border: 1px solid var(--border);
  padding: 4px 8px;
  white-space: nowrap;
  max-width: 420px;
  overflow: hidden;
  text-overflow: ellipsis;
  background: var(--bg-cell);
}
.grid tr.alt td { background: var(--bg-cell-alt); }
.grid thead th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--bg-header);
  font-weight: 600;
  vertical-align: top;
}
.grid .row-num, .grid thead .row-num {
  color: var(--dim);
  text-align: right;
  width: 44px;
  max-width: 56px;
  cursor: pointer;
}
.grid td.sel { background: var(--selected) !important; }
.sort {
  background: transparent;
  border: none;
  color: #fff;
  font-weight: 600;
  padding: 0;
  text-align: left;
  width: 100%;
}
.sort:hover { color: #cce6ff; }
.filter {
  width: 100%;
  margin-top: 4px;
  background: var(--input);
  border: 1px solid var(--border);
  padding: 3px 6px;
  border-radius: 3px;
  font-size: 12px;
}
.empty { color: var(--dim); padding: 24px; text-align: center; }
.status.info { color: var(--info); }
.status.err { color: var(--error); }
.status.ok { color: var(--success); }
"#;
