use std::{cell::RefCell, collections::HashMap, process::Child};

use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained, runtime::Sel, sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSImage, NSMenu, NSMenuItem,
    NSSquareStatusItemLength, NSStatusBar, NSStatusItem,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSString, ns_string};

use super::{close_panel, open_panel};
use crate::{config, terminal};

#[derive(Default)]
struct TrayTargetIvars {
    status_item: RefCell<Option<Retained<NSStatusItem>>>,
    panel_process: RefCell<Option<Child>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and TrayTarget has no Drop impl.
    // MainThreadOnly confines all ivars and AppKit calls to the application main thread.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = TrayTargetIvars]
    struct TrayTarget;

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for TrayTarget {}

    impl TrayTarget {
        // SAFETY: This matches -[NSObject connect:] with one object argument and a void return.
        #[unsafe(method(connect:))]
        fn connect(&self, sender: &NSMenuItem) {
            let Some(host) = sender
                .representedObject()
                .and_then(|object| object.downcast::<NSString>().ok())
            else {
                return;
            };
            let host = host.to_string();
            if let Err(error) = terminal::open_ssh(&host) {
                eprintln!("Failed to open SSH session for {host:?}: {error}");
            }
        }

        // SAFETY: This matches a target/action method with one object argument and void return.
        #[unsafe(method(refresh:))]
        fn refresh(&self, _sender: &NSMenuItem) {
            let menu = build_menu(self.mtm(), self);
            if let Some(status_item) = self.ivars().status_item.borrow().as_ref() {
                status_item.setMenu(Some(&menu));
            }
        }

        // SAFETY: This matches a target/action method with one object argument and void return.
        #[unsafe(method(openPanel:))]
        fn open_panel(&self, _sender: &NSMenuItem) {
            if let Err(error) = open_panel(&mut self.ivars().panel_process.borrow_mut()) {
                eprintln!("Failed to open SSH panel: {error}");
            }
        }

        // SAFETY: This matches a target/action method with one object argument and void return.
        #[unsafe(method(quit:))]
        fn quit(&self, _sender: &NSMenuItem) {
            close_panel(&mut self.ivars().panel_process.borrow_mut());
            if let Some(status_item) = self.ivars().status_item.borrow_mut().take() {
                NSStatusBar::systemStatusBar().removeStatusItem(&status_item);
            }
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl TrayTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TrayTargetIvars::default());
        // SAFETY: This invokes NSObject's designated no-argument initializer on our allocation.
        unsafe { msg_send![super(this), init] }
    }
}

fn menu_item(
    mtm: MainThreadMarker,
    title: &str,
    enabled: bool,
    action: Option<Sel>,
    target: Option<&TrayTarget>,
) -> Retained<NSMenuItem> {
    let title = NSString::from_str(title);
    // SAFETY: action is either None or a selector implemented by TrayTarget. The target is
    // retained by run for the entire event loop, outliving every menu whose weak target is set.
    let item = unsafe {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &title,
            action,
            ns_string!(""),
        );
        if let Some(target) = target {
            item.setTarget(Some(target));
        }
        item
    };
    item.setEnabled(enabled);
    item
}

fn action_item(
    mtm: MainThreadMarker,
    title: &str,
    action: Sel,
    target: &TrayTarget,
) -> Retained<NSMenuItem> {
    menu_item(mtm, title, true, Some(action), Some(target))
}

fn build_menu(mtm: MainThreadMarker, target: &TrayTarget) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    menu.addItem(&menu_item(mtm, "SSH Tray", false, None, None));

    let status = match config::load_hosts() {
        Ok(hosts) if hosts.is_empty() => Some("No SSH hosts configured".to_string()),
        Ok(hosts) => {
            let status = format!("{} hosts · Manual refresh", hosts.len());
            menu.addItem(&menu_item(mtm, &status, false, None, None));
            menu.addItem(&NSMenuItem::separatorItem(mtm));

            let mut groups: HashMap<String, Retained<NSMenuItem>> = HashMap::new();
            let mut group_order = Vec::new();

            for host in hosts {
                let config::Host { alias, group, .. } = host;
                let item = action_item(mtm, &alias, sel!(connect:), target);
                let represented_alias = NSString::from_str(&alias);
                // SAFETY: represented_alias is an NSString, the type expected by connect:.
                unsafe {
                    item.setRepresentedObject(Some(&represented_alias));
                }

                if let Some(group) = group {
                    let group_item = groups.entry(group.clone()).or_insert_with(|| {
                        group_order.push(group.clone());
                        let item = menu_item(mtm, &group, true, None, None);
                        let submenu =
                            NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(&group));
                        item.setSubmenu(Some(&submenu));
                        submenu.setAutoenablesItems(false);
                        item
                    });
                    group_item
                        .submenu()
                        .expect("group menu item must have a submenu")
                        .addItem(&item);
                } else {
                    menu.addItem(&item);
                }
            }

            for group in group_order {
                let group_item = groups.remove(&group).expect("ordered group must exist");
                menu.addItem(&group_item);
            }
            None
        }
        Err(error) => {
            eprintln!("Failed to load SSH hosts: {error}");
            Some(format!("SSH config error: {error}"))
        }
    };

    if let Some(text) = status {
        menu.addItem(&menu_item(mtm, &text, false, None, None));
    }

    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&action_item(mtm, "Refresh", sel!(refresh:), target));
    menu.addItem(&action_item(mtm, "Open Panel", sel!(openPanel:), target));
    menu.addItem(&action_item(mtm, "Quit", sel!(quit:), target));
    menu
}

pub(super) fn run() -> Result<(), String> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "SSH tray must be started on the main thread".to_string())?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let target = TrayTarget::new(mtm);
    let menu = build_menu(mtm, &target);
    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSSquareStatusItemLength);

    let data = NSData::with_bytes(include_bytes!("../../assets/tray-icon.png"));
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "failed to load tray icon: invalid PNG data".to_string())?;
    image.setTemplate(true);

    let button = status_item
        .button(mtm)
        .ok_or_else(|| "failed to create tray icon: status item has no button".to_string())?;
    button.setImage(Some(&image));
    button.setToolTip(Some(ns_string!("SSH Tray")));
    status_item.setMenu(Some(&menu));
    *target.ivars().status_item.borrow_mut() = Some(status_item);

    app.run();

    close_panel(&mut target.ivars().panel_process.borrow_mut());
    if let Some(status_item) = target.ivars().status_item.borrow_mut().take() {
        status_bar.removeStatusItem(&status_item);
    }
    Ok(())
}
