#![windows_subsystem = "windows"]

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontId, RichText, TextStyle};
use modded_among_us::game::{detect_installs, GameInstall, Platform};
use modded_among_us::mods::{self, Choice, ModSet, LATEST};

const CLOSE_AFTER: Duration = Duration::from_secs(3);
const BG: Color32 = Color32::from_rgb(21, 22, 28);
const CARD: Color32 = Color32::from_rgb(31, 33, 41);
const ACCENT: Color32 = Color32::from_rgb(200, 40, 60);
const MUTED: Color32 = Color32::from_rgb(150, 154, 168);
const GOOD: Color32 = Color32::from_rgb(90, 200, 120);
const WARN: Color32 = Color32::from_rgb(235, 180, 70);

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
            .with_inner_size([500.0, 470.0])
            .with_resizable(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../Assets/Icon.png"))
                    .expect("Assets/Icon.png is a valid PNG"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Modded Among Us",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
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
            _ => "Pick a version and press Play.",
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

    /// Draw the store chips and the selected install path.
    ///
    /// @param ui: egui UI.
    /// @return: None
    fn draw_game(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "GAME");
        ui.horizontal(|ui| {
            for i in 0..self.installs.len() {
                let name = store_name(self.installs[i].platform);
                let text = RichText::new(name).size(15.0).strong();
                if ui.selectable_label(self.selected == i, text).clicked() && self.selected != i {
                    self.selected = i;
                    self.choice = mods::load_choice(&self.installs[i].dir);
                }
            }
        });
        let game = &self.installs[self.selected];
        ui.label(
            RichText::new(format!("{}  ({})", game.dir.display(), game.arch()))
                .small()
                .color(MUTED),
        );
    }

    /// Draw the mod toggle, version picker and notes for the picked version.
    ///
    /// @param ui: egui UI.
    /// @return: None
    fn draw_mods(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "MODS");
        ui.checkbox(
            &mut self.choice.enabled,
            RichText::new("Load Town of Us Mira").size(15.0),
        );
        let dir = self.installs[self.selected].dir.clone();
        let arch = self.installs[self.selected].arch();
        let saved: Vec<String> = mods::archived_versions(&dir)
            .into_iter()
            .chain(mods::installed_version(&dir))
            .collect();
        let loaded = self.versions[self.selected].lock().unwrap().clone();
        let list: Vec<ModSet> = match &loaded {
            Some(Ok(list)) => list.clone(),
            _ => Vec::new(),
        };
        let mut tags: Vec<String> = list.iter().map(|v| v.tou_tag.clone()).collect();
        tags.extend(
            saved
                .iter()
                .filter(|t| !tags.contains(t))
                .cloned()
                .collect::<Vec<_>>(),
        );
        let latest = mods::latest(&list).map(|v| v.tou_tag.clone());
        let label = |tag: &str| -> String {
            let mut text = match tag {
                LATEST => format!("{LATEST} ({})", latest.as_deref().unwrap_or("???")),
                _ => tag.to_string(),
            };
            let set = list.iter().find(|v| v.tou_tag == tag);
            if set.is_some_and(|v| v.prerelease) {
                text.push_str("   pre-release");
            }
            if set.is_some_and(|v| !v.fits) {
                text.push_str("   not built for ");
                text.push_str(arch);
            }
            if set.is_some_and(|v| v.too_old) {
                text.push_str("   older than your game");
            }
            if saved.iter().any(|s| s == tag) {
                text.push_str("   downloaded");
            }
            text
        };
        ui.add_space(4.0);
        ui.add_enabled_ui(self.choice.enabled, |ui| {
            egui::ComboBox::from_id_salt("version")
                .width(ui.available_width())
                .height(320.0)
                .selected_text(RichText::new(label(&self.choice.version)).size(15.0))
                .show_ui(ui, |ui| {
                    for tag in std::iter::once(LATEST.to_string()).chain(tags.iter().cloned()) {
                        let text = label(&tag);
                        ui.selectable_value(&mut self.choice.version, tag, text);
                    }
                });
        });
        let wanted = if self.choice.version == LATEST {
            latest.clone()
        } else {
            Some(self.choice.version.clone())
        };
        let picked = wanted.and_then(|tag| list.iter().find(|v| v.tou_tag == tag).cloned());
        ui.horizontal_wrapped(|ui| match (&loaded, &picked) {
            (None, _) => {
                ui.spinner();
                ui.label(RichText::new("Loading versions...").color(MUTED));
            }
            (Some(Err(e)), _) => {
                ui.label(RichText::new(format!("Offline: {e}")).small().color(WARN));
            }
            (_, Some(set)) => {
                if !set.notes_url.is_empty() {
                    ui.hyperlink_to("Release notes", &set.notes_url);
                }
                if !set.fits {
                    ui.label(
                        RichText::new(format!(
                            "This build is not made for {arch} games and may not load."
                        ))
                        .small()
                        .color(WARN),
                    );
                }
                if set.too_old {
                    ui.label(
                        RichText::new(
                            "This build came out before your Among Us version and will likely crash.",
                        )
                        .small()
                        .color(WARN),
                    );
                }
            }
            _ => {}
        });
    }

    /// Draw the progress bar and status text.
    ///
    /// @param ui: egui UI.
    /// @param phase: Current phase.
    /// @param line: Status text.
    /// @param fraction: Progress 0..1.
    /// @return: None
    fn draw_status(&self, ui: &mut egui::Ui, phase: Phase, line: &str, fraction: f32) {
        if phase == Phase::Working {
            ui.add(
                egui::ProgressBar::new(fraction)
                    .fill(ACCENT)
                    .show_percentage(),
            );
        }
        let color = match phase {
            Phase::Failed => ACCENT,
            Phase::Launched => GOOD,
            _ => MUTED,
        };
        ui.label(RichText::new(line).color(color));
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
        let panel = egui::Frame::none().fill(BG).inner_margin(20.0);
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            ui.label(
                RichText::new("Modded Among Us")
                    .size(28.0)
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.label(RichText::new("Town of Us Mira launcher").color(MUTED));
            ui.add_space(12.0);
            if !self.installs.is_empty() {
                ui.add_enabled_ui(phase != Phase::Working, |ui| {
                    card(ui, |ui| self.draw_game(ui));
                    ui.add_space(10.0);
                    card(ui, |ui| self.draw_mods(ui));
                });
                ui.add_space(10.0);
            }
            self.draw_status(ui, phase, &line, fraction);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                let can_play = list_ready && matches!(phase, Phase::Waiting | Phase::Failed);
                let text = RichText::new("PLAY")
                    .size(20.0)
                    .strong()
                    .color(Color32::WHITE);
                let button = egui::Button::new(text).fill(ACCENT).rounding(8.0);
                play = ui
                    .add_enabled(
                        can_play,
                        button.min_size(egui::vec2(ui.available_width(), 48.0)),
                    )
                    .clicked();
            });
        });
        if play {
            self.start();
        }
        if phase == Phase::Launched
            && self.launched_at.get_or_insert_with(Instant::now).elapsed() > CLOSE_AFTER
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if matches!(phase, Phase::Working | Phase::Launched) || !list_ready {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

/// Set the dark theme, accent color, spacing and font sizes.
///
/// @param ctx: egui context.
/// @return: None
fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = BG;
    visuals.selection.bg_fill = ACCENT;
    visuals.hyperlink_color = Color32::from_rgb(120, 170, 255);
    visuals.widgets.inactive.rounding = 6.0.into();
    visuals.widgets.hovered.rounding = 6.0.into();
    visuals.widgets.active.rounding = 6.0.into();
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(14.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(12.0));
    });
}

/// Draw content inside a rounded card.
///
/// @param ui: egui UI.
/// @param add: Draws the card content.
/// @return: None
fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(CARD)
        .rounding(10.0)
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// Draw a small muted section heading.
///
/// @param ui: egui UI.
/// @param text: Heading text.
/// @return: None
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().strong().color(MUTED));
}

/// Full store name for the UI.
///
/// @param platform: Store.
/// @return: Display name.
fn store_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Steam => "Steam",
        Platform::Epic => "Epic Games",
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
        *out.lock().unwrap() = Some(mods::list_versions(
            game.platform,
            game.arch(),
            game.version(),
        ))
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
            mods::latest(&versions)
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
