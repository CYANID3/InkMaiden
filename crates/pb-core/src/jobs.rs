//! Фоновые задачи: прогресс и отмена. Окно получает события, кнопка «Стоп» ставит флаг.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);
impl Cancel {
    pub fn new() -> Cancel { Cancel::default() }
    pub fn cancel(&self) { self.0.store(true, Ordering::SeqCst) }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::SeqCst) }
}

#[derive(Default)]
pub struct Jobs { map: Mutex<HashMap<String, Cancel>> }
impl Jobs {
    pub fn start(&self, id: &str) -> Cancel {
        let c = Cancel::new();
        if let Some(old) = self.map.lock().unwrap().insert(id.to_string(), c.clone()) { old.cancel() }
        c
    }
    pub fn cancel(&self, id: &str) { if let Some(c) = self.map.lock().unwrap().get(id) { c.cancel() } }
    pub fn finish(&self, id: &str) { self.map.lock().unwrap().remove(id); }
    /// что сейчас идёт (для «Безопасного извлечения»)
    pub fn active(&self) -> Vec<String> { self.map.lock().unwrap().keys().cloned().collect() }
}
