//! Tray icon, the flyout window and the global shortcut.
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const W: f64 = 360.0;
const H: f64 = 600.0;

struct Last {
    /// Tray icon rectangle (x, y, w, h) in physical pixels, from the last click.
    rect: Option<(f64, f64, f64, f64)>,
    hidden_at: Option<Instant>,
}
static LAST: Mutex<Last> = Mutex::new(Last { rect: None, hidden_at: None });

pub fn show_main(app: &AppHandle, route: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(r) = route {
            let _ = app.emit_to("main", "navigate", r);
        }
    }
    hide_flyout(app);
}

pub fn hide_flyout(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("tray") {
        let _ = w.hide();
    }
}

/// Opens the flyout above the tray icon, or in the bottom-right corner when the icon's position is unknown.
pub fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window("tray") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    // Clicking the icon blurs (and so hides) the flyout first; don't reopen it straight away.
    let just_hidden = LAST.lock().ok().and_then(|l| l.hidden_at).is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if just_hidden {
        return;
    }
    let scale = w.scale_factor().unwrap_or(1.0);
    let (ww, wh) = (W * scale, H * scale);
    let screen = w.primary_monitor().ok().flatten().map(|m| (m.size().width as f64, m.size().height as f64)).unwrap_or((1920.0, 1080.0));
    let rect = LAST.lock().ok().and_then(|l| l.rect);
    let (x, y) = match rect {
        Some((rx, ry, rw, rh)) => {
            let x = (rx + rw / 2.0 - ww / 2.0).clamp(8.0, (screen.0 - ww - 8.0).max(8.0));
            let y = if ry > screen.1 / 2.0 { ry - wh - 8.0 } else { ry + rh + 8.0 };
            (x, y.max(8.0))
        }
        None => (screen.0 - ww - 12.0, screen.1 - wh - 60.0 * scale),
    };
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to("tray", "flyout-shown", ());
}

pub fn set_visible(app: &AppHandle, on: bool) {
    if let Some(t) = app.tray_by_id("main") {
        let _ = t.set_visible(on);
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let flyout = WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("index.html#tray".into()))
        .title("Tandem")
        .inner_size(W, H)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    let handle = app.clone();
    flyout.on_window_event(move |e| {
        if let WindowEvent::Focused(false) = e {
            if let Some(w) = handle.get_webview_window("tray") {
                if w.is_visible().unwrap_or(false) {
                    if let Ok(mut l) = LAST.lock() {
                        l.hidden_at = Some(Instant::now());
                    }
                    let _ = w.hide();
                }
            }
        }
    });

    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Tandem", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = if cfg!(target_os = "macos") { Menu::with_items(app, &[&open, &settings, &quit])? } else { Menu::with_items(app, &[&open, &quit])? };
    let mut b = TrayIconBuilder::with_id("main").tooltip("Tandem").menu(&menu).show_menu_on_left_click(false);
    // A template image is drawn by macOS in the menu bar's own colour, so no theme swap is needed.
    #[cfg(target_os = "macos")]
    {
        b = b.icon_as_template(true);
    }
    if let Some(i) = app.default_window_icon() {
        b = b.icon(i.clone());
    }
    b.on_menu_event(|app, e| match e.id.as_ref() {
        "open" => show_main(app, None),
        "settings" => show_main(app, Some("settings")),
        _ => app.exit(0),
    })
    .on_tray_icon_event(|t, e| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = e {
            let scale = t.app_handle().get_webview_window("tray").and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
            let (p, s) = (rect.position.to_physical::<f64>(scale), rect.size.to_physical::<f64>(scale));
            if let Ok(mut l) = LAST.lock() {
                l.rect = Some((p.x, p.y, s.width, s.height));
            }
            toggle(t.app_handle());
        }
    })
    .build(app)?;
    Ok(())
}

/// Swap the tray icon to match the app theme.
pub fn set_icon(app: &AppHandle, dark: bool) {
    if cfg!(target_os = "macos") {
        return;
    }
    let bytes: &[u8] = if dark { include_bytes!("../../public/icon-dark.png") } else { include_bytes!("../../public/icon-light.png") };
    if let (Some(t), Ok(img)) = (app.tray_by_id("main"), tauri::image::Image::from_bytes(bytes)) {
        let _ = t.set_icon(Some(img));
    }
}

/// macOS app menu: the standard one plus "Settings…" on ⌘, (the Mac convention).
#[cfg(target_os = "macos")]
pub fn app_menu(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::MenuItemKind;
    let menu = Menu::default(app)?;
    if let Some(MenuItemKind::Submenu(s)) = menu.items()?.into_iter().next() {
        s.insert(&MenuItem::with_id(app, "app-settings", "Settings…", true, Some("CmdOrCtrl+,"))?, 2)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, e| {
        if e.id().as_ref() == "app-settings" {
            show_main(app, Some("settings"));
        }
    });
    Ok(())
}
