use std::collections::HashMap;

#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
};

use crate::{config, terminal};

enum AppEvent {
    Menu(MenuEvent),
    Wake,
}

struct TrayMenu {
    menu: Menu,
    hosts: HashMap<MenuId, String>,
    refresh: MenuItem,
    open_panel: MenuItem,
    quit: MenuItem,
}

fn build_menu() -> Result<TrayMenu, String> {
    let menu = Menu::new();
    let mut host_ids = HashMap::new();

    menu.append(&MenuItem::new("SSH Tray", false, None))
        .map_err(|error| format!("failed to add app header: {error}"))?;

    let status = match config::load_hosts() {
        Ok(hosts) if hosts.is_empty() => Some("No SSH hosts configured".to_string()),
        Ok(hosts) => {
            let status = format!("{} hosts · Manual refresh", hosts.len());
            menu.append(&MenuItem::new(status, false, None))
                .map_err(|error| format!("failed to add host status: {error}"))?;
            menu.append(&PredefinedMenuItem::separator())
                .map_err(|error| format!("failed to add host separator: {error}"))?;

            let mut groups = HashMap::new();
            let mut group_order = Vec::new();

            for host in hosts {
                let config::Host { alias, group, .. } = host;
                let item = MenuItem::new(&alias, true, None);
                if let Some(group) = group {
                    let submenu = groups.entry(group.clone()).or_insert_with(|| {
                        group_order.push(group.clone());
                        Submenu::new(&group, true)
                    });
                    submenu.append(&item).map_err(|error| {
                        format!("failed to add host {alias:?} to group {group:?}: {error}")
                    })?;
                } else {
                    menu.append(&item).map_err(|error| {
                        format!("failed to add host {alias:?} to menu: {error}")
                    })?;
                }
                host_ids.insert(item.id().clone(), alias);
            }

            for group in group_order {
                let submenu = groups.remove(&group).expect("ordered group must exist");
                menu.append(&submenu)
                    .map_err(|error| format!("failed to add group {group:?}: {error}"))?;
            }
            None
        }
        Err(error) => {
            eprintln!("Failed to load SSH hosts: {error}");
            Some(format!("SSH config error: {error}"))
        }
    };

    if let Some(text) = status {
        menu.append(&MenuItem::new(text, false, None))
            .map_err(|error| format!("failed to add status to menu: {error}"))?;
    }

    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| format!("failed to add menu separator: {error}"))?;

    let refresh = MenuItem::new("Refresh", true, None);
    menu.append(&refresh)
        .map_err(|error| format!("failed to add Refresh to menu: {error}"))?;

    let open_panel = MenuItem::new("Open Panel", true, None);
    menu.append(&open_panel)
        .map_err(|error| format!("failed to add Open Panel to menu: {error}"))?;

    let quit = MenuItem::new("Quit", true, None);
    menu.append(&quit)
        .map_err(|error| format!("failed to add Quit to menu: {error}"))?;

    Ok(TrayMenu {
        menu,
        hosts: host_ids,
        refresh,
        open_panel,
        quit,
    })
}

fn open_panel(process: &mut Option<std::process::Child>) -> Result<(), String> {
    let running = match process.as_mut() {
        Some(child) => match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(error) => return Err(format!("failed to inspect panel process: {error}")),
        },
        None => false,
    };
    if running {
        return Ok(());
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate ssh-tray executable: {error}"))?;
    *process = Some(
        std::process::Command::new(executable)
            .arg("--panel")
            .spawn()
            .map_err(|error| format!("failed to open panel: {error}"))?,
    );
    Ok(())
}

fn close_panel(process: &mut Option<std::process::Child>) {
    if let Some(mut child) = process.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn run() -> Result<(), String> {
    let icon = Icon::from_rgba(include_bytes!("../assets/tray-icon.rgba").to_vec(), 22, 22)
        .map_err(|error| format!("failed to load tray icon: {error}"))?;
    let mut current_menu = build_menu()?;

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    let mut event_loop = event_loop;
    #[cfg(target_os = "macos")]
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let menu_proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(AppEvent::Menu(event));
    }));
    let wake_proxy = event_loop.create_proxy();

    let mut tray_icon = None;
    let mut panel_process = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                match TrayIconBuilder::new()
                    .with_menu(Box::new(current_menu.menu.clone()))
                    .with_menu_on_left_click(true)
                    .with_tooltip("SSH Tray")
                    .with_icon(icon.clone())
                    .build()
                {
                    Ok(icon) => {
                        tray_icon = Some(icon);
                        // Wake Tao once more so AppKit displays the newly created status item.
                        let _ = wake_proxy.send_event(AppEvent::Wake);
                    }
                    Err(error) => {
                        eprintln!("Failed to create tray icon: {error}");
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            Event::UserEvent(AppEvent::Menu(event)) => {
                if event.id() == current_menu.quit.id() {
                    close_panel(&mut panel_process);
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                } else if event.id() == current_menu.refresh.id() {
                    match build_menu() {
                        Ok(new_menu) => {
                            if let Some(tray) = tray_icon.as_ref() {
                                tray.set_menu(Some(Box::new(new_menu.menu.clone())));
                            }
                            current_menu = new_menu;
                        }
                        Err(error) => eprintln!("Failed to rebuild SSH menu: {error}"),
                    }
                } else if event.id() == current_menu.open_panel.id() {
                    if let Err(error) = open_panel(&mut panel_process) {
                        eprintln!("Failed to open SSH panel: {error}");
                    }
                } else if let Some(host) = current_menu.hosts.get(event.id())
                    && let Err(error) = terminal::open_ssh(host)
                {
                    eprintln!("Failed to open SSH session for {host:?}: {error}");
                }
            }
            Event::UserEvent(AppEvent::Wake) => {}
            _ => {}
        }
    })
}
