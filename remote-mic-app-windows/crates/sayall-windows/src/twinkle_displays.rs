//! Parsing and target selection for the public Twinkle Tray --List interface.
use serde::Serialize;

pub const MAX_LIST_BYTES: usize = 65536;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Display {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSnapshot {
    pub is_all: bool,
    pub name: String,
}
#[derive(Default)]
pub struct Selection(Option<Display>);
impl Selection {
    pub const fn new() -> Self {
        Self(None)
    }
    pub fn id(&self) -> Option<&str> {
        self.0.as_ref().map(|d| d.id.as_str())
    }
    pub fn snapshot(&self) -> TargetSnapshot {
        TargetSnapshot {
            is_all: self.0.is_none(),
            name: self
                .0
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "所有显示器".into()),
        }
    }
    pub fn reset(&mut self) {
        self.0 = None;
    }
    pub fn cycle(&mut self, displays: &[Display]) -> Result<(), String> {
        if displays.is_empty() {
            return Err("没有找到可调节亮度的显示器，请检查 Twinkle Tray".into());
        }
        let next = match self.id() {
            None => displays.first(),
            Some(id) => displays
                .iter()
                .position(|d| d.id == id)
                .and_then(|i| displays.get(i + 1)),
        };
        self.0 = next.cloned();
        Ok(())
    }
}

pub fn parse_displays(output: &str) -> Result<Vec<Display>, String> {
    let invalid = || "无法识别显示器列表，请检查 Twinkle Tray 版本后重试".to_owned();
    if output.len() > MAX_LIST_BYTES {
        return Err(invalid());
    }
    let mut clean = String::new();
    let mut chars = output.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() != Some('[') {
                return Err(invalid());
            }
            let mut ended = false;
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    ended = true;
                    break;
                }
            }
            if !ended {
                return Err(invalid());
            }
        } else {
            clean.push(c);
        }
    }
    if !clean.contains("MonitorNum:") {
        return Err(invalid());
    }
    let mut displays: Vec<Display> = Vec::new();
    for block in clean.split("MonitorNum:").skip(1) {
        let field = |key: &str| -> Result<&str, String> {
            let values: Vec<_> = block
                .lines()
                .filter_map(|l| l.trim().strip_prefix(key).map(str::trim))
                .collect();
            if values.len() != 1 || values[0].is_empty() {
                return Err(invalid());
            }
            Ok(values[0])
        };
        let kind = field("Type:")?;
        if !matches!(kind, "ddcci" | "wmi") {
            continue;
        }
        let id = field("MonitorID:")?;
        let name = field("Name:")?;
        if id.len() > 1024
            || name.len() > 1024
            || id.chars().any(char::is_control)
            || displays.iter().any(|d| d.id.eq_ignore_ascii_case(id))
        {
            return Err(invalid());
        }
        displays.push(Display {
            id: id.to_owned(),
            name: name.to_owned(),
        });
    }
    // --MonitorID accepts substring matches. Reject overlapping keys rather than
    // silently sending a request to the first of two possible physical displays.
    for (index, display) in displays.iter().enumerate() {
        let id = display.id.to_lowercase();
        if displays
            .iter()
            .enumerate()
            .any(|(other, d)| other != index && d.id.to_lowercase().contains(&id))
        {
            return Err(invalid());
        }
    }
    let original_names: Vec<_> = displays.iter().map(|d| d.name.clone()).collect();
    for (index, display) in displays.iter_mut().enumerate() {
        if original_names
            .iter()
            .filter(|name| **name == display.name)
            .count()
            > 1
        {
            display.name = format!("{}（{}）", display.name, index + 1);
        }
    }
    Ok(displays)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn displays() -> Vec<Display> {
        parse_displays("\x1b[36mMonitorNum:\x1b[0m 0\nMonitorID: ignored\nName: 显示器\nBrightness: 50\nType: none\n\nMonitorNum: 1\nMonitorID: alpha\nName: 屏幕 A\nBrightness: 11\nType: ddcci\n\nMonitorNum: 2\nMonitorID: beta\nName: 屏幕 B\nBrightness: 40\nType: wmi\n").unwrap()
    }
    #[test]
    fn parses_unicode_and_ansi_and_filters_unsupported_displays() {
        let d = displays();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].name, "屏幕 A");
        assert_eq!(d[1].id, "beta");
    }
    #[test]
    fn duplicate_names_are_distinguishable_and_substring_ids_are_rejected() {
        let text = "MonitorNum: 1\nMonitorID: alpha\nName: Same\nType: ddcci\nMonitorNum: 2\nMonitorID: beta\nName: Same\nType: ddcci";
        let displays = parse_displays(text).unwrap();
        assert_eq!(displays[0].name, "Same（1）");
        assert_eq!(displays[1].name, "Same（2）");
        assert!(parse_displays(&text.replace("beta", "alpha-long")).is_err());
    }

    #[test]
    fn cycle_is_all_then_each_display_then_all_without_relying_on_numbers() {
        let d = displays();
        let mut selected = Selection::default();
        assert!(selected.snapshot().is_all);
        selected.cycle(&d).unwrap();
        assert_eq!(selected.id(), Some("alpha"));
        selected.cycle(&d).unwrap();
        assert_eq!(selected.id(), Some("beta"));
        selected.cycle(&d).unwrap();
        assert!(selected.snapshot().is_all);
    }
    #[test]
    fn reorder_keeps_identity_and_missing_target_resets_to_all() {
        let mut d = displays();
        let mut selected = Selection::default();
        selected.cycle(&d).unwrap();
        d.reverse();
        assert_eq!(selected.id(), Some("alpha"));
        d.retain(|x| x.id != "alpha");
        selected.cycle(&d).unwrap();
        assert!(selected.snapshot().is_all);
    }
    #[test]
    fn rejects_ambiguous_truncated_and_oversized_output_without_changing_selection() {
        assert!(parse_displays("").is_err());
        assert!(parse_displays("MonitorNum: 1\nName: missing\nType: ddcci").is_err());
        assert!(parse_displays(&"x".repeat(65537)).is_err());
        let duplicate = "MonitorNum: 1\nMonitorID: same\nName: A\nType: ddcci\nMonitorNum: 2\nMonitorID: same\nName: B\nType: ddcci";
        assert!(parse_displays(duplicate).is_err());
        let mut s = Selection::default();
        s.cycle(&displays()).unwrap();
        assert!(s.cycle(&[]).is_err());
        assert_eq!(s.id(), Some("alpha"));
    }
}
