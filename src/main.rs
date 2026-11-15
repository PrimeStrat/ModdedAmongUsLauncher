#![windows_subsystem = "windows"]

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use modded_among_us::game::{detect_installs, GameInstall};
use modded_among_us::mods::{self, Choice, ModSet, LATEST};

const CLOSE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Waiting,
    Working,
    Launched,
    Failed,
}

struct Status {
    phase: Phase,
    line: String,
    fraction: f32,
}

type VersionList = Arc<Mutex<Option<Result<Vec<ModSet>, String>>>>;

struct App {
    installs: Vec<GameInstall>,
    versions: Vec<VersionList>,
    selected: usize,
    choice: Choice,
    launched_at: Option<Instant>,
    status: Arc<Mutex<Status>>,
}

/// Open the launcher window.
///
/// @return: Ok when the window closes normally.
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 300.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "Modded Among Us",
        options,
        Box::new(|_| Ok(Box::new(App::new()))),
    )
}

impl App {
    /// Detect installs and start loading their version lists.
    ///
    /// @return: The app state.
    fn new() -> Self {
        let installs = detect_installs();
        let versions: Vec<VersionList> = installs
            .iter()
            .map(|game| load_versions(game.clone()))
            .collect();
        let line = match installs.len() {
            0 => "Among Us was not found in Steam or Epic Games. Install it from your store first.",
            1 => "Pick a version and press Play.",
            _ => "Among Us was found in more than one store. Pick one and press Play.",
        };
        App {
            choice: installs
                .first()
                .map(|g| mods::load_choice(&g.dir))
                .unwrap_or(Choice {
                    enabled: true,
                    version: LATEST.into(),
                }),
            installs,
            versions,
            selected: 0,
            launched_at: None,
            status: Arc::new(Mutex::new(Status {
                phase: Phase::Waiting,
                line: line.into(),
                fraction: 0.0,
            })),
        }
    }

    /// Switch the game folder to the chosen version and launch, on a worker thread.
    ///
    /// @return: None
    fn start(&mut self) {
        let game = self.installs[self.selected].clone();
        let list = self.versions[self.selected].lock().unwrap().clone();
        let choice = self.choice.clone();
        let status = self.status.clone();
        set_status(&status, Phase::Working, "Preparing...".into(), 0.0);
        thread::spawn(move || {
            let progress =
                |line: String, fraction: f32| set_status(&status, Phase::Working, line, fraction);
            match prepare_and_launch(&game, &choice, list, &progress) {
                Ok(()) => set_status(
                    &status,
                    Phase::Launched,
                    "Launching Among Us...".into(),
                    1.0,
                ),
                Err(e) => set_status(&status, Phase::Failed, e, 0.0),
            }
        });
    }

    /// Draw the store picker, mod toggle and version picker.
    ///
    /// @param ui: egui UI.
    /// @param busy: True while a switch is running.
    /// @return: None
    fn draw_controls(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.add_enabled_ui(!busy, |ui| {
            for i in 0..self.installs.len() {
                let game = &self.installs[i];
                let text = format!("{}  ({})", game.platform.name(), game.dir.display());
                if ui.radio(self.selected == i, text).clicked() && self.selected != i {
                    self.selected = i;
                    self.choice = mods::load_choice(&self.installs[i].dir);
                }
            }
            if self.installs.is_empty() {
                return;
            }
            ui.add_space(6.0);
            ui.checkbox(&mut self.choice.enabled, "Load mods");
            let dir = self.installs[self.selected].dir.clone();
            let saved: Vec<String> = mods::archived_versions(&dir)
                .into_iter()
                .chain(mods::installed_version(&dir))
                .collect();
            let mut tags: Vec<String> = match &*self.versions[self.selected].lock().unwrap() {
                Some(Ok(list)) => list.iter().map(|v| v.tou_tag.clone()).collect(),
                _ => Vec::new(),
            };
            tags.extend(
                saved
                    .iter()
                    .filter(|t| !tags.contains(t))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            let label = |tag: &str| {
                let shown = if tag == LATEST {
                    format!("{LATEST} ({})", tags.first().map_or("???", String::as_str))
                } else {
                    tag.to_string()
                };
                if saved.iter().any(|s| s == tag) {
                    format!("{shown}  (downloaded)")
                } else {
                    shown
                }
            };
            ui.add_enabled_ui(self.choice.enabled, |ui| {
                egui::ComboBox::from_label("Version")
                    .selected_text(label(&self.choice.version))
                    .show_ui(ui, |ui| {
                        for tag in std::iter::once(LATEST.to_string()).chain(tags.iter().cloned()) {
                            let text = label(&tag);
                            ui.selectable_value(&mut self.choice.version, tag, text);
                        }
                    });
            });
        });
    }
}

impl eframe::App for App {
    /// Draw the window each frame and close it after the game launches.
    ///
    /// @param ctx: egui context.
    /// @param _frame: eframe frame.
    /// @return: None
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (phase, line, fraction) = {
            let s = self.status.lock().unwrap();
            (s.phase, s.line.clone(), s.fraction)
        };
        let list_ready = self
            .versions
            .get(self.selected)
            .is_some_and(|v| v.lock().unwrap().is_some());
        let mut play = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Modded Among Us");
            ui.label("Town of Us Mira");
            ui.add_space(8.0);
            self.draw_controls(ui, phase == Phase::Working);
            ui.add_space(8.0);
            if !list_ready && !self.installs.is_empty() {
                ui.label("Loading versions...");
            }
            if phase == Phase::Working {
                ui.add(egui::ProgressBar::new(fraction).show_percentage());
            }
            ui.label(&line);
            let can_play = list_ready && matches!(phase, Phase::Waiting | Phase::Failed);
            play = can_play && ui.button("Play").clicked();
        });
        if play {
            self.start();
        }
        if phase == Phase::Launched
            && self.launched_at.get_or_insert_with(Instant::now).elapsed() > CLOSE_AFTER
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

/// Fetch the version list for one install on a worker thread.
///
/// @param game: Install to list versions for.
/// @return: Shared slot that fills once the list is loaded.
fn load_versions(game: GameInstall) -> VersionList {
    let slot: VersionList = Arc::new(Mutex::new(None));
    let out = slot.clone();
    thread::spawn(move || {
        *out.lock().unwrap() = Some(mods::list_versions(game.platform, game.arch()))
    });
    slot
}

/// Save the choice, make the game folder match it, then start the game.
///
/// @param game: Install to use.
/// @param choice: Mods on or off and the wanted version.
/// @param list: Loaded version list, or an error when offline.
/// @param progress: Called with a status line and a 0..1 fraction.
/// @return: Ok once the game launch was started.
fn prepare_and_launch(
    game: &GameInstall,
    choice: &Choice,
    list: Option<Result<Vec<ModSet>, String>>,
    progress: &dyn Fn(String, f32),
) -> Result<(), String> {
    let versions = match list {
        Some(Ok(v)) => v,
        _ => Vec::new(),
    };
    let wanted = match (choice.enabled, choice.version.as_str()) {
        (false, _) => None,
        (true, LATEST) => Some(
            versions
                .first()
                .map(|v| v.tou_tag.clone())
                .or_else(|| mods::installed_version(&game.dir))
                .ok_or("Could not find the latest version. Check your internet.")?,
        ),
        (true, tag) => Some(tag.to_string()),
    };
    mods::save_choice(&game.dir, choice)?;
    mods::switch_to(&game.dir, wanted.as_deref(), &versions, progress)?;
    game.launch()
}

/// Replace the shared status.
///
/// @param status: Shared status.
/// @param phase: New phase.
/// @param line: Status text.
/// @param fraction: Progress 0..1.
/// @return: None
fn set_status(status: &Mutex<Status>, phase: Phase, line: String, fraction: f32) {
    *status.lock().unwrap() = Status {
        phase,
        line,
        fraction,
    };
}
