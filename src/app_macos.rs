//! macOS only: small window behaviours Mac users expect, other than the menu bar
//! (that is in app_macos_menu.rs).

use objc2_app_kit::NSView;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Shows or clears the unsaved-changes dot in the window's close button (and "Edited" by the
/// title). Returns false if the window wasn't available yet, so the caller can try again.
pub(super) fn set_document_edited(frame: &eframe::Frame, edited: bool) -> bool {
    let Ok(handle) = frame.window_handle() else { return false };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return false };
    // Safety: eframe gives us the live NSView of its window, and we are on the main thread.
    let view: &NSView = unsafe { h.ns_view.cast().as_ref() };
    let Some(window) = view.window() else { return false };
    window.setDocumentEdited(edited);
    true
}

/// Control-click is the Mac right-click: turn a primary press made with Control (and not ⌘) into
/// a secondary one, along with its release. `held` remembers a converted press between frames.
pub(super) fn control_click_as_right_click(raw: &mut egui::RawInput, held: &mut bool) {
    for e in raw.events.iter_mut() {
        if let egui::Event::PointerButton { button, pressed, modifiers, .. } = e {
            if *button != egui::PointerButton::Primary {
                continue;
            }
            if *pressed {
                *held = modifiers.ctrl && !modifiers.mac_cmd;
            }
            if *held {
                *button = egui::PointerButton::Secondary;
            }
            if !*pressed {
                *held = false;
            }
        }
    }
}

// ---------------------------------------------------------------- files opened from Finder
//
// Double-clicking a .dia file, dropping one on the Dock icon or "Open With" doesn't pass the file
// on the command line: macOS sends the running app an "open documents" Apple Event, which winit
// doesn't handle. We install our own handler for it, once AppKit has set up its defaults
// (NSApplicationWillFinishLaunching), and queue the paths for the next frame.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, ClassType};
use objc2_foundation::{NSNotificationCenter, NSURL};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// 'aevt', 'odoc' and '----' (the event class, event ID and direct-object keyword).
const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

static OPENED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static REPAINT: OnceLock<egui::Context> = OnceLock::new();

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "ChartCraftOpenHandler"]
    struct OpenHandler;

    impl OpenHandler {
        #[unsafe(method(willFinishLaunching:))]
        fn will_finish_launching(&self, _note: &AnyObject) {
            unsafe {
                let manager: Retained<AnyObject> = msg_send![objc2::class!(NSAppleEventManager), sharedAppleEventManager];
                let _: () = msg_send![&*manager, setEventHandler: self, andSelector: sel!(handleOpen:withReply:), forEventClass: CORE_EVENT_CLASS, andEventID: OPEN_DOCUMENTS];
            }
        }

        #[unsafe(method(handleOpen:withReply:))]
        fn handle_open(&self, event: &AnyObject, _reply: &AnyObject) {
            let mut paths = Vec::new();
            unsafe {
                let list: Option<Retained<AnyObject>> = msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT];
                let Some(list) = list else { return };
                let n: isize = msg_send![&*list, numberOfItems];
                // A single file may come as a plain descriptor rather than a list of one.
                let items: Vec<Retained<AnyObject>> = if n == 0 { vec![list.clone()] } else { (1..=n).filter_map(|i| msg_send![&*list, descriptorAtIndex: i]).collect() };
                for d in items {
                    let url: Option<Retained<NSURL>> = msg_send![&*d, fileURLValue];
                    if let Some(path) = url.and_then(|u| u.path()) {
                        paths.push(PathBuf::from(path.to_string()));
                    }
                }
            }
            if let Ok(mut q) = OPENED.lock() {
                q.extend(paths);
            }
            if let Some(ctx) = REPAINT.get() {
                ctx.request_repaint();
            }
        }
    }
);

/// Call in main(), before the app starts running.
pub(crate) fn install_open_handler() {
    let handler: Retained<OpenHandler> = unsafe { msg_send![OpenHandler::class(), new] };
    unsafe {
        NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
            &handler,
            sel!(willFinishLaunching:),
            Some(objc2_app_kit::NSApplicationWillFinishLaunchingNotification),
            None,
        );
    }
    // Neither the notification centre nor the Apple Event manager keeps it alive.
    std::mem::forget(handler);
}

/// Lets a file opened from Finder wake the app (call once the app exists).
pub(super) fn set_repaint(ctx: &egui::Context) {
    let _ = REPAINT.set(ctx.clone());
}

/// Files Finder asked us to open since the last frame.
pub(super) fn take_opened() -> Vec<PathBuf> {
    OPENED.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

