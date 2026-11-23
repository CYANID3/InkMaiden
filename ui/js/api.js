// Связь окна с программой на Rust. Внутри InkMaiden — команды Tauri; в обычном браузере — проверочные данные (dev/mock.js),
// чтобы интерфейс можно было открыть и проверить без сборки.
const T = window.__TAURI__;
let mock = null;
async function getMock() { if (!mock) mock = await import('../../dev/mock.js'); return mock; }


export async function call(cmd, args = {}) {
  if (T) return T.core.invoke(cmd, args);
  return (await getMock()).call(cmd, args);
}

export async function on(event, cb) {
  if (T) return T.event.listen(event, e => cb(e.payload));
  return (await getMock()).on(event, cb);
}

export async function pickFolder(title, defaultPath) {
  if (T) return T.core.invoke('plugin:dialog|open', { options: { directory: true, title, defaultPath: defaultPath || undefined } });
  return (await getMock()).pickFolder();
}

/** выбрать файлы (несколько) */
export async function pickFiles(title, defaultPath) {
  if (T) { const r = await T.core.invoke('plugin:dialog|open', { options: { multiple: true, title, defaultPath: defaultPath || undefined } }); return r ? (Array.isArray(r) ? r : [r]) : []; }
  return window.__mockFiles || ['C:\\Загрузки\\Пикник на обочине.fb2', 'C:\\Загрузки\\Аудио\\Глава 1.mp3'];
}

export async function openPath(path) {
  if (T) return T.core.invoke('open_folder', { path });
}

// перетаскивание папки в окно
export function onDrop(cb, hover) {
  if (T) {
    T.event.listen('tauri://drag-enter', () => hover(true));
    T.event.listen('tauri://drag-leave', () => hover(false));
    T.event.listen('tauri://drag-drop', e => { hover(false); const p = e.payload && e.payload.paths; if (p && p.length) cb(p); });
  } else {
    addEventListener('dragover', e => { e.preventDefault(); hover(true); });
    addEventListener('dragleave', () => hover(false));
    addEventListener('drop', e => { e.preventDefault(); hover(false); cb([window.__mockDrop || 'C:\\Комиксы\\Пример']); });
  }
}
