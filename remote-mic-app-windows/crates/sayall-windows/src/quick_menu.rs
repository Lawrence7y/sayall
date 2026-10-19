//! A remote-only four-way menu. No keyboard focus or third-party state is read.
use crate::{raw_input::RemoteButton, send_input::ButtonAction};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const MENU_TIMEOUT: Duration = Duration::from_secs(10);
pub fn is_direction(button: RemoteButton) -> bool {
    matches!(
        button,
        RemoteButton::Up | RemoteButton::Down | RemoteButton::Left | RemoteButton::Right
    )
}
pub fn is_control(button: RemoteButton) -> bool {
    is_direction(button) || matches!(button, RemoteButton::Ok | RemoteButton::Back)
}
#[derive(Clone, Debug, Serialize)]
pub struct Presentation {
    pub generation: u64,
    pub labels: BTreeMap<RemoteButton, String>,
}
type Listener = Arc<dyn Fn(Option<Presentation>) + Send + Sync>;
static LISTENER: Mutex<Option<Listener>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(1);
static CURRENT_PRESENTATION: AtomicU64 = AtomicU64::new(0);
pub fn is_current_presentation(generation: u64) -> bool {
    generation != 0 && CURRENT_PRESENTATION.load(Ordering::Acquire) == generation
}
pub fn set_listener(listener: Option<Listener>) {
    *LISTENER.lock().unwrap_or_else(|e| e.into_inner()) = listener;
}
pub fn publish(presentation: Option<Presentation>) {
    CURRENT_PRESENTATION.store(
        presentation
            .as_ref()
            .map(|item| item.generation)
            .unwrap_or(0),
        Ordering::Release,
    );
    let listener = LISTENER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(listener) = listener {
        listener(presentation);
    }
}
#[derive(Debug)]
struct Session {
    actions: BTreeMap<RemoteButton, ButtonAction>,
    deadline: Instant,
    generation: u64,
    ready: bool,
}
#[derive(Debug, Default)]
pub struct QuickMenu {
    session: Option<Session>,
}
impl QuickMenu {
    pub fn open(&mut self, actions: BTreeMap<RemoteButton, ButtonAction>, now: Instant) {
        self.session = Some(Session {
            actions,
            deadline: now + MENU_TIMEOUT,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            ready: false,
        });
    }
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.session.as_ref().map(|s| s.deadline)
    }
    pub fn generation(&self) -> Option<u64> {
        self.session.as_ref().map(|s| s.generation)
    }
    pub fn mark_ready(&mut self, generation: u64) {
        if let Some(session) = &mut self.session {
            if session.generation == generation {
                session.ready = true;
            }
        }
    }
    pub fn close(&mut self) -> bool {
        self.session.take().is_some()
    }
    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            self.close()
        } else {
            false
        }
    }
    /// None: ordinary key. Some(None): cancelled or unconfigured. Some(Some): one action.
    pub fn press(&mut self, button: RemoteButton) -> Option<Option<ButtonAction>> {
        if !is_control(button) || !self.is_open() {
            return None;
        }
        let session = self.session.take().unwrap();
        let action = session
            .actions
            .get(&button)
            .cloned()
            .filter(|a| *a != ButtonAction::Disabled);
        Some(if session.ready && is_direction(button) {
            action
        } else {
            None
        })
    }
    pub fn presentation(&self) -> Option<Presentation> {
        self.presentation_with_apps(&[])
    }

    /// Menu-only names from the user's own application library, never a launch
    /// target rewrite or a registry/filesystem query on the input thread.
    pub fn presentation_with_apps(
        &self,
        applications: &[crate::app_launcher::CustomAppPick],
    ) -> Option<Presentation> {
        let session = self.session.as_ref()?;
        let labels = [
            RemoteButton::Up,
            RemoteButton::Down,
            RemoteButton::Left,
            RemoteButton::Right,
        ]
        .into_iter()
        .map(|button| {
            let label = session
                .actions
                .get(&button)
                .map(|action| match action {
                    ButtonAction::OpenApp { target } => compact_app_name(target, applications),
                    _ => action_label(action),
                })
                .unwrap_or_else(|| "未设置".into());
            (button, label)
        })
        .collect();
        Some(Presentation {
            generation: session.generation,
            labels,
        })
    }
}
fn action_label(action: &ButtonAction) -> String {
    match action {
        ButtonAction::Disabled => "未设置".into(),
        ButtonAction::Shortcut { chord } => chord
            .keys
            .iter()
            .map(key_label)
            .collect::<Vec<_>>()
            .join(" + "),
        ButtonAction::TwinkleCycleMonitor => "切换显示器".into(),
        ButtonAction::TwinkleBrightness { direction, .. }
        | ButtonAction::TwinkleBrightnessSelected { direction, .. } => match direction {
            crate::send_input::ScrollDirection::Up => "调亮显示器".into(),
            crate::send_input::ScrollDirection::Down => "调暗显示器".into(),
        },
        ButtonAction::OpenApp { target } => compact_app_name(target, &[]),
        ButtonAction::FocusInput => "聚焦输入框".into(),
        ButtonAction::Scroll { direction, .. } => match direction {
            crate::send_input::ScrollDirection::Up => "向上滚动".into(),
            crate::send_input::ScrollDirection::Down => "向下滚动".into(),
        },
        ButtonAction::MouseClick { .. } => "鼠标点击".into(),
        ButtonAction::MouseMove { direction, .. } => match direction {
            crate::send_input::MoveDirection::Up => "光标上移".into(),
            crate::send_input::MoveDirection::Down => "光标下移".into(),
            crate::send_input::MoveDirection::Left => "光标左移".into(),
            crate::send_input::MoveDirection::Right => "光标右移".into(),
        },
        ButtonAction::FourWayMenu { .. } => "四向快捷菜单".into(),
    }
}
fn compact_app_name(target: &str, applications: &[crate::app_launcher::CustomAppPick]) -> String {
    let name = if let Some(app) = applications
        .iter()
        .find(|app| app.path.eq_ignore_ascii_case(target))
    {
        app.name.as_str()
    } else if let Some(app) = crate::app_launcher::preset_app(target) {
        app.name
    } else if crate::registered_apps::is_registered_target(target) {
        // An unknown AUMID is not a human-readable application name.
        "应用"
    } else {
        let file = target.rsplit(['\\', '/']).next().unwrap_or("应用");
        match file.rsplit_once('.') {
            Some((stem, extension))
                if extension.eq_ignore_ascii_case("exe")
                    || extension.eq_ignore_ascii_case("lnk") =>
            {
                stem
            }
            _ => file,
        }
    };
    let name = name.trim();
    match name.to_ascii_lowercase().as_str() {
        "deepseek harness" | "dsh" => "DSH".into(),
        "visual studio code" | "microsoft visual studio code" | "vs code" | "code" => {
            "VS Code".into()
        }
        "edge 浏览器" | "microsoft edge" | "msedge" => "Edge".into(),
        "chrome 浏览器" | "google chrome" => "Chrome".into(),
        "文件资源管理器" | "windows 资源管理器" | "file explorer" | "explorer" => {
            "资源管理器".into()
        }
        "性能监视器" | "performance monitor" | "perfmon" => "性能监视".into(),
        "chatgpt" => "ChatGPT".into(),
        "" => "应用".into(),
        _ => name.into(),
    }
}

fn key_label(key: &crate::send_input::KeyCode) -> String {
    let encoded = serde_json::to_value(key).unwrap_or_default();
    let name = encoded.as_str().unwrap_or("");
    match name {
        "control" => "Ctrl".into(),
        "left_control" => "左 Ctrl".into(),
        "right_control" => "右 Ctrl".into(),
        "shift" => "Shift".into(),
        "alt" => "Alt".into(),
        "left_win" | "right_win" => "Win".into(),
        "volume_up" => "音量+".into(),
        "volume_down" => "音量−".into(),
        "volume_mute" => "静音".into(),
        "space" => "空格".into(),
        "escape" => "Esc".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "media_play_pause" => "播放/暂停".into(),
        "media_next_track" => "下一首".into(),
        "media_prev_track" => "上一首".into(),
        _ if name.starts_with("digit") => name.trim_start_matches("digit").into(),
        _ if name.len() == 1 => name.to_uppercase(),
        _ => format!("{key:?}"),
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn app_labels_are_short_names_without_launch_verbs() {
        for (target, expected) in [
            ("wechat", "微信"),
            ("edge", "Edge"),
            ("vscode", "VS Code"),
            ("explorer", "资源管理器"),
        ] {
            assert_eq!(
                action_label(&ButtonAction::OpenApp {
                    target: target.into()
                }),
                expected
            );
        }
    }

    use super::*;

    #[test]
    fn menu_uses_library_names_and_aliases_without_changing_launch_targets() {
        let apps = vec![
            crate::app_launcher::CustomAppPick {
                name: "DeepSeek Harness".into(),
                path: r"shell:AppsFolder\Fixture.Harness!App".into(),
            },
            crate::app_launcher::CustomAppPick {
                name: "ChatGPT".into(),
                path: r"shell:AppsFolder\Fixture.ChatGPT!App".into(),
            },
            crate::app_launcher::CustomAppPick {
                name: "性能监视器".into(),
                path: r"C:\Fixture\PERFMON.EXE".into(),
            },
        ];
        let actions = BTreeMap::from([
            (
                RemoteButton::Right,
                ButtonAction::OpenApp {
                    target: apps[0].path.clone(),
                },
            ),
            (
                RemoteButton::Left,
                ButtonAction::OpenApp {
                    target: apps[1].path.clone(),
                },
            ),
            (
                RemoteButton::Down,
                ButtonAction::OpenApp {
                    target: apps[2].path.clone(),
                },
            ),
        ]);
        let mut menu = QuickMenu::default();
        menu.open(actions.clone(), Instant::now());
        let presentation = menu.presentation_with_apps(&apps).unwrap();
        assert_eq!(presentation.labels[&RemoteButton::Right], "DSH");
        assert_eq!(presentation.labels[&RemoteButton::Left], "ChatGPT");
        assert_eq!(presentation.labels[&RemoteButton::Down], "性能监视");
        menu.mark_ready(presentation.generation);
        assert_eq!(
            menu.press(RemoteButton::Right),
            Some(actions.get(&RemoteButton::Right).cloned())
        );
        assert_eq!(apps[0].name, "DeepSeek Harness");
        assert_eq!(apps[0].path, r"shell:AppsFolder\Fixture.Harness!App");
    }

    #[test]
    fn app_name_fallbacks_never_show_an_aumid_and_preserve_unknown_brands() {
        assert_eq!(
            compact_app_name(r"shell:AppsFolder\Fixture.Unknown!App", &[]),
            "应用"
        );
        assert_eq!(
            compact_app_name(r"C:\Fixture\Unique Brand.EXE", &[]),
            "Unique Brand"
        );
        assert_eq!(compact_app_name(r"C:\Fixture\Code.LNK", &[]), "VS Code");
    }

    #[test]
    fn closing_or_replacing_presentation_invalidates_late_window_callbacks() {
        publish(Some(Presentation {
            generation: 100001,
            labels: BTreeMap::new(),
        }));
        assert!(is_current_presentation(100001));
        publish(None);
        assert!(!is_current_presentation(100001));
        publish(Some(Presentation {
            generation: 100002,
            labels: BTreeMap::new(),
        }));
        assert!(!is_current_presentation(100001));
        assert!(is_current_presentation(100002));
        publish(None);
    }
    #[test]
    fn one_direction_executes_once_cancel_timeout_and_missing_actions_are_safe() {
        let now = Instant::now();
        let mut menu = QuickMenu::default();
        let action = ButtonAction::TwinkleCycleMonitor;
        let actions = BTreeMap::from([(RemoteButton::Up, action.clone())]);
        menu.open(actions.clone(), now);
        menu.mark_ready(menu.generation().unwrap());
        assert_eq!(menu.press(RemoteButton::Up), Some(Some(action)));
        assert_eq!(menu.press(RemoteButton::Up), None);
        for button in [RemoteButton::Ok, RemoteButton::Back, RemoteButton::Down] {
            menu.open(actions.clone(), now);
            menu.mark_ready(menu.generation().unwrap());
            assert_eq!(menu.press(button), Some(None));
            assert!(!menu.is_open());
        }
        menu.open(actions, now);
        assert_eq!(menu.press(RemoteButton::VolumeUp), None);
        assert!(!menu.expire(now + MENU_TIMEOUT - Duration::from_millis(1)));
        assert!(menu.expire(now + MENU_TIMEOUT));
        assert!(!menu.close());
    }
    #[test]
    fn hidden_menu_and_late_ready_cannot_execute_an_action() {
        let mut menu = QuickMenu::default();
        let actions = BTreeMap::from([(RemoteButton::Up, ButtonAction::TwinkleCycleMonitor)]);
        menu.open(actions.clone(), Instant::now());
        let old = menu.generation().unwrap();
        menu.open(actions, Instant::now());
        menu.mark_ready(old);
        assert_eq!(menu.press(RemoteButton::Up), Some(None));
        assert!(!menu.is_open());
    }
    #[test]
    fn reopening_has_a_new_generation_and_four_labels() {
        let mut menu = QuickMenu::default();
        menu.open(BTreeMap::new(), Instant::now());
        let first = menu.presentation().unwrap();
        assert_eq!(first.labels.len(), 4);
        menu.open(BTreeMap::new(), Instant::now());
        assert_ne!(first.generation, menu.presentation().unwrap().generation);
    }
}
