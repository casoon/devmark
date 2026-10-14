//! Synchronisierung der Notizen über einen Sync-Ordner (opt-in).
//!
//! Läuft im Hintergrund mit eigener Datenbankverbindung (WAL erlaubt parallele Zugriffe),
//! damit ein langsamer Cloud-Ordner die Oberfläche nicht blockiert.

use std::path::PathBuf;
use std::time::Duration;

use devmark_notes::create_backup;
use devmark_storage::Store;
use devmark_sync::{FolderTransport, SyncReport, set_last_sync, sync};
use gpui_kit::component::{WindowExt, notification::Notification};
use gpui_kit::*;

use crate::notes_ui::data_dir;
use crate::{ChooseSyncFolder, DisableSync, SyncNow, Workspace};

/// Automatischer Abgleich, solange die App läuft.
const SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
pub struct SyncSettings {
    pub folder: Option<PathBuf>,
    pub running: bool,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    /// Vor dem ersten Abgleich dieser Sitzung wird ein Backup angelegt.
    pub backed_up: bool,
}

fn settings_file() -> PathBuf {
    data_dir().join("sync-folder")
}

impl SyncSettings {
    pub fn load() -> Self {
        let folder = std::fs::read_to_string(settings_file())
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| !p.as_os_str().is_empty());
        Self { folder, ..Default::default() }
    }

    fn save(&self) {
        match &self.folder {
            Some(folder) => {
                std::fs::write(settings_file(), folder.to_string_lossy().as_bytes()).ok();
            }
            None => {
                std::fs::remove_file(settings_file()).ok();
            }
        }
    }
}

impl Workspace {
    /// Beim Start: Abgleich, falls eingerichtet, danach periodisch.
    pub(crate) fn start_sync_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.folder.is_none() {
            return;
        }
        self.sync_now(&SyncNow, window, cx);
        self.sync_timer = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(SYNC_INTERVAL).await;
                let alive = this.update_in(cx, |this, window, cx| this.sync_now(&SyncNow, window, cx));
                if alive.is_err() {
                    break;
                }
            }
        });
    }

    pub(crate) fn choose_sync_folder(&mut self, _: &ChooseSyncFolder, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Als Sync-Ordner verwenden".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                this.sync.folder = Some(folder.clone());
                this.sync.save();
                window.push_notification(
                    Notification::info(format!(
                        "Sync-Ordner: {}. Andere Geräte wählen denselben (über iCloud Drive, Nextcloud, Dropbox …) verteilten Ordner.",
                        folder.display()
                    )),
                    cx,
                );
                this.start_sync_timer(window, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn disable_sync(&mut self, _: &DisableSync, window: &mut Window, cx: &mut Context<Self>) {
        self.sync.folder = None;
        self.sync.save();
        self.sync_timer = Task::ready(());
        window.push_notification(Notification::info("Synchronisierung ausgeschaltet. Die Daten im Sync-Ordner bleiben erhalten."), cx);
        cx.notify();
    }

    pub(crate) fn sync_now(&mut self, _: &SyncNow, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.sync.folder.clone() else {
            self.choose_sync_folder(&ChooseSyncFolder, window, cx);
            return;
        };
        if self.sync.running || self.notes.is_none() {
            return;
        }
        // Offene Notizen zuerst sichern, damit Editor und Datenbank übereinstimmen.
        self.save_all_notes(cx);
        if !self.sync.backed_up {
            if let Some(state) = self.notes.as_ref() {
                let dir = data_dir().join("Backups");
                if let Err(err) = create_backup(state.notes.store(), &dir, 14) {
                    window.push_notification(
                        Notification::error(format!("Backup vor dem Sync fehlgeschlagen – Sync abgebrochen: {err}")),
                        cx,
                    );
                    return;
                }
            }
            self.sync.backed_up = true;
        }
        self.sync.running = true;
        cx.notify();

        let db_path = data_dir().join("devmark.sqlite");
        let task = cx.background_spawn(async move {
            let mut store = Store::open(&db_path).map_err(|e| e.to_string())?;
            let report = sync(&mut store, &FolderTransport::new(&folder)).map_err(|e| e.to_string())?;
            let now = devmark_storage::now_ms();
            set_last_sync(&store, now).map_err(|e| e.to_string())?;
            Ok::<(SyncReport, i64), String>((report, now))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.sync.running = false;
                match result {
                    Ok((report, at)) => {
                        this.sync.last_sync = Some(at);
                        this.sync.last_error = None;
                        this.apply_sync_report(&report, window, cx);
                    }
                    Err(err) => {
                        // Nur einmal melden, nicht alle fünf Minuten.
                        if this.sync.last_error.as_deref() != Some(err.as_str()) {
                            window.push_notification(Notification::error(format!("Synchronisierung fehlgeschlagen: {err}")), cx);
                        }
                        this.sync.last_error = Some(err);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Offene Notizen an fremde Änderungen anpassen und Ergebnis melden.
    fn apply_sync_report(&mut self, report: &SyncReport, window: &mut Window, cx: &mut Context<Self>) {
        for id in &report.changed_documents {
            let document = self.notes.as_ref().and_then(|s| s.notes.get(id).ok().flatten());
            for pane in &mut self.panes {
                let Some(ix) = pane.buffers.iter().position(|b| b.note_id.as_deref() == Some(id.as_str())) else {
                    continue;
                };
                if pane.buffers[ix].dirty {
                    continue; // eigene, noch ungesicherte Änderung – wird beim nächsten Sync zur Konfliktkopie
                }
                match &document {
                    Some(document) if document.deleted_at.is_none() => {
                        let current = pane.buffers[ix].editor.read(cx).value().to_string();
                        if current != document.content {
                            let content = document.content.clone();
                            pane.buffers[ix].editor.update(cx, |editor, cx| editor.set_value(content, window, cx));
                            pane.buffers[ix].title = Some(document.display_title());
                        }
                    }
                    _ => {
                        pane.buffers.remove(ix);
                        pane.active = pane.active.min(pane.buffers.len().saturating_sub(1));
                    }
                }
            }
        }
        if !report.conflicts.is_empty() {
            window.push_notification(
                Notification::warning(format!(
                    "Gleichzeitig auf zwei Geräten geändert – die andere Fassung liegt als eigene Notiz daneben: {}",
                    report.conflicts.join(", ")
                )),
                cx,
            );
        }
        if report.pulled + report.deleted_local > 0 {
            self.refresh_notes(cx);
        }
    }
}
