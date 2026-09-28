use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone)]
pub enum TrayAction {
    Show(String),
    Close(String),
    Quit,
}

type ActionMap = Arc<Mutex<HashMap<MenuId, TrayAction>>>;

pub struct Tray {
    icon: TrayIcon,
    action_map: ActionMap,
}

impl Tray {
    pub fn new(action_tx: Sender<TrayAction>, ctx: egui::Context) -> Self {
        let action_map: ActionMap = Arc::new(Mutex::new(HashMap::new()));

        {
            let action_map = action_map.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let action = action_map.lock().unwrap().get(&event.id).cloned();
                if let Some(action) = action {
                    let _ = action_tx.send(action);
                    // MenuEvent fires off the egui thread; while the window is
                    // hidden and idle, nothing else would wake eframe to drain it.
                    ctx.request_repaint();
                }
            }));
        }

        let menu = build_menu(&[], &action_map);
        let icon = TrayIconBuilder::new()
            .with_tooltip("termview")
            .with_icon(placeholder_icon())
            .with_menu(Box::new(menu))
            .build()
            .expect("failed to build tray icon");

        Self { icon, action_map }
    }

    /// Rebuild the menu to reflect the current set of persistent sessions.
    /// `tray-icon` menu item ids are only known after construction, so the whole
    /// menu (and the id -> action lookup table it shares with the MenuEvent
    /// handler) has to be rebuilt whenever the persistent-session set changes.
    pub fn rebuild(&mut self, persistent_workspaces: &[String]) {
        let menu = build_menu(persistent_workspaces, &self.action_map);
        let _ = self.icon.set_menu(Some(Box::new(menu)));
    }
}

fn build_menu(persistent_workspaces: &[String], action_map: &ActionMap) -> Menu {
    let menu = Menu::new();
    let mut map = action_map.lock().unwrap();
    map.clear();

    for workspace in persistent_workspaces {
        let item = MenuItem::new(workspace, true, None);
        map.insert(item.id().clone(), TrayAction::Show(workspace.clone()));
        let _ = menu.append(&item);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    let close_submenu = Submenu::new("Close Session", true);
    for workspace in persistent_workspaces {
        let item = MenuItem::new(workspace, true, None);
        map.insert(item.id().clone(), TrayAction::Close(workspace.clone()));
        let _ = close_submenu.append(&item);
    }
    let _ = menu.append(&close_submenu);

    let _ = menu.append(&PredefinedMenuItem::separator());

    let quit_item = MenuItem::new("Quit", true, None);
    map.insert(quit_item.id().clone(), TrayAction::Quit);
    let _ = menu.append(&quit_item);

    drop(map);
    menu
}

// Flat Nord4-ish square — swap for a real bundled asset later if desired.
fn placeholder_icon() -> Icon {
    let size = 16u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for _ in 0..(size * size) {
        rgba.extend_from_slice(&[216, 222, 233, 255]);
    }
    Icon::from_rgba(rgba, size, size).expect("failed to build tray icon bitmap")
}
