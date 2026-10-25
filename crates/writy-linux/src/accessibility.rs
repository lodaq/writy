//! AT-SPI inspection and verified edits run on a dedicated worker.
//! Text-bearing snapshots intentionally do not implement `Debug`.

use anyhow::{Result, anyhow, ensure};
use gtk::glib::{ffi as glib_ffi, gobject_ffi};
use std::{
    collections::HashSet,
    ffi::{CStr, CString, c_char, c_int, c_uint, c_void},
    ptr::{self, NonNull},
    sync::{OnceLock, mpsc},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use writy_core::types::Suggestion;

const MAX_FIELD_CHARACTERS: usize = 8192;
const OPERATION_LIMIT: Duration = Duration::from_millis(1500);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone)]
pub struct FieldSnapshot {
    pub identity: String,
    pub text: String,
    pub caret: usize,
    pub rect: Option<Rect>,
}

pub struct Observation {
    pub secure: bool,
    pub field: Option<FieldSnapshot>,
    pub status: String,
}

impl Observation {
    fn unknown(status: &str) -> Self {
        Self {
            secure: false,
            field: None,
            status: status.into(),
        }
    }
}

#[derive(Clone)]
pub struct Accessibility {
    sender: mpsc::SyncSender<Request>,
}

enum Request {
    Inspect {
        pid: u32,
        deadline: Instant,
        reply: oneshot::Sender<Observation>,
    },
    Replace {
        pid: u32,
        suggestion: Box<Suggestion>,
        expected: FieldSnapshot,
        deadline: Instant,
        reply: oneshot::Sender<Result<()>>,
    },
}

impl Accessibility {
    pub fn start() -> Self {
        static WORKER: OnceLock<mpsc::SyncSender<Request>> = OnceLock::new();
        let sender = WORKER.get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel(2);
            // Failure leaves a disconnected channel and gives callers an unknown
            // observation instead of taking down the desktop application.
            let _ = std::thread::Builder::new()
                .name("writy-accessibility".into())
                .spawn(move || worker(receiver));
            sender
        });
        Self {
            sender: sender.clone(),
        }
    }

    pub async fn inspect(&self, pid: u32) -> Observation {
        if pid == 0 {
            return Observation::unknown("Application process is unknown");
        }
        let (reply, receiver) = oneshot::channel();
        if self
            .sender
            .try_send(Request::Inspect {
                pid,
                deadline: Instant::now() + OPERATION_LIMIT,
                reply,
            })
            .is_err()
        {
            return Observation::unknown("Accessibility worker is busy or unavailable");
        }
        match tokio::time::timeout(Duration::from_secs(2), receiver).await {
            Ok(Ok(observation)) => observation,
            _ => Observation::unknown("Accessibility inspection timed out"),
        }
    }

    pub async fn verify_replace(
        &self,
        pid: u32,
        suggestion: Suggestion,
        expected: FieldSnapshot,
    ) -> Result<()> {
        ensure!(pid != 0, "Application process is unknown; use Copy");
        let (reply, receiver) = oneshot::channel();
        self.sender
            .try_send(Request::Replace {
                pid,
                suggestion: Box::new(suggestion),
                expected,
                deadline: Instant::now() + OPERATION_LIMIT,
                reply,
            })
            .map_err(|_| anyhow!("Accessibility worker is busy or unavailable; use Copy"))?;
        tokio::time::timeout(Duration::from_secs(2), receiver)
            .await
            .map_err(|_| {
                anyhow!("Accessibility replacement timed out; inspect the field before retrying")
            })?
            .map_err(|_| anyhow!("Accessibility worker stopped; use Copy"))?
    }
}

fn worker(receiver: mpsc::Receiver<Request>) {
    // All AT-SPI objects, calls, and GLib event dispatch remain on this thread.
    let context = unsafe { glib_ffi::g_main_context_new() };
    unsafe {
        glib_ffi::g_main_context_push_thread_default(context);
        ffi::atspi_set_main_context(context);
        ffi::atspi_set_timeout(150, 150);
    }
    let initialized = matches!(unsafe { ffi::atspi_init() }, 0 | 1);
    loop {
        for _ in 0..32 {
            if unsafe { glib_ffi::g_main_context_iteration(context, 0) } == 0 {
                break;
            }
        }
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(Request::Inspect {
                pid,
                deadline,
                reply,
            }) => {
                if reply.is_closed() {
                    continue;
                }
                let observation = if !initialized {
                    Observation::unknown("Accessibility service is unavailable")
                } else {
                    inspect_field(pid, deadline)
                };
                let _ = reply.send(observation);
            }
            Ok(Request::Replace {
                pid,
                suggestion,
                expected,
                deadline,
                reply,
            }) => {
                if reply.is_closed() {
                    continue;
                }
                let result = if initialized {
                    replace_field(pid, &suggestion, &expected, deadline, &reply)
                } else {
                    Err(anyhow!("Accessibility service is unavailable; use Copy"))
                };
                let _ = reply.send(result);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    unsafe {
        if initialized {
            ffi::atspi_exit();
        }
        glib_ffi::g_main_context_pop_thread_default(context);
        glib_ffi::g_main_context_unref(context);
    }
}

fn inspect_field(pid: u32, deadline: Instant) -> Observation {
    let result = (|| -> Result<Observation> {
        let Some(focused) = find_focused(pid, deadline)? else {
            return Ok(Observation::unknown(
                "No focused accessible field for this process",
            ));
        };
        if focused.is_secure()? {
            return Ok(Observation {
                secure: true,
                field: None,
                status: "Password field; assistance disabled".into(),
            });
        }
        let field = focused.snapshot(pid, deadline)?;
        Ok(Observation {
            secure: false,
            field: Some(field),
            status: "Accessible text and caret available".into(),
        })
    })();
    result.unwrap_or_else(|_| Observation::unknown("Focused field cannot be verified"))
}

fn replace_field(
    pid: u32,
    suggestion: &Suggestion,
    expected: &FieldSnapshot,
    deadline: Instant,
    reply: &oneshot::Sender<Result<()>>,
) -> Result<()> {
    ensure!(
        expected.identity.starts_with(&format!("{pid}:")),
        "The application process changed; use Copy"
    );
    let focused = find_focused(pid, deadline)?
        .ok_or_else(|| anyhow!("The focused field is unavailable; use Copy"))?;
    let current = focused.snapshot(pid, deadline)?;
    let replacement = verified_replacement(
        expected,
        &current,
        &suggestion.snapshot.text,
        suggestion.start,
        suggestion.end,
        &suggestion.replacement,
    )
    .ok_or_else(|| anyhow!("The original field, text, or caret changed; use Copy"))?;
    let replacement = CString::new(replacement)
        .map_err(|_| anyhow!("Replacement contains unsupported text; use Copy"))?;
    let editable = object_call(|error| {
        let _ = error;
        unsafe { ffi::atspi_accessible_get_editable_text_iface(focused.ptr()) }
    })?
    .ok_or_else(|| anyhow!("The field does not support verified replacement; use Copy"))?;

    // Re-read after constructing the replacement, and never run an edit for a
    // cancelled caller. AT-SPI has no atomic compare-and-set: the final app-side
    // read/write race cannot be eliminated by this interface.
    let latest = focused.snapshot(pid, deadline)?;
    ensure!(
        verified_replacement(
            expected,
            &latest,
            &suggestion.snapshot.text,
            suggestion.start,
            suggestion.end,
            &suggestion.replacement
        )
        .is_some(),
        "The field changed before replacement; use Copy"
    );
    ensure!(
        !reply.is_closed() && Instant::now() < deadline,
        "Replacement request expired; use Copy"
    );
    let changed = checked(|error| unsafe {
        ffi::atspi_editable_text_set_text_contents(editable.ptr(), replacement.as_ptr(), error)
    })?;
    ensure!(
        changed != 0,
        "The application refused replacement; use Copy"
    );
    Ok(())
}

fn find_focused(pid: u32, deadline: Instant) -> Result<Option<Object>> {
    ensure!(Instant::now() < deadline, "Accessibility request expired");
    let Some(desktop) = Object::from_full(unsafe { ffi::atspi_get_desktop(0) }) else {
        return Ok(None);
    };
    let count =
        checked(|error| unsafe { ffi::atspi_accessible_get_child_count(desktop.ptr(), error) })?
            .clamp(0, 256);
    let mut pending = Vec::new();
    for i in 0..count {
        ensure!(Instant::now() < deadline, "Accessibility request expired");
        let Some(app) = object_call(|error| unsafe {
            ffi::atspi_accessible_get_child_at_index(desktop.ptr(), i, error)
        })?
        else {
            continue;
        };
        if checked(|error| unsafe { ffi::atspi_accessible_get_process_id(app.ptr(), error) }).ok()
            == Some(pid)
        {
            unsafe {
                ffi::atspi_accessible_set_cache_mask(app.ptr(), 0);
            }
            pending.push((app, 0));
        }
    }
    let mut seen = HashSet::new();
    let mut examined = 0;
    while let Some((node, depth)) = pending.pop() {
        ensure!(Instant::now() < deadline, "Accessibility request expired");
        examined += 1;
        if examined > 512 {
            break;
        }
        let identity = node.identity(pid)?;
        if !seen.insert(identity) {
            continue;
        }
        node.refresh();
        if node.has_state(ffi::STATE_FOCUSED)?
            && (node.is_secure()? || node.has_state(ffi::STATE_EDITABLE)?)
        {
            return Ok(Some(node));
        }
        if depth >= 32 {
            continue;
        }
        let count =
            checked(|error| unsafe { ffi::atspi_accessible_get_child_count(node.ptr(), error) })?
                .clamp(0, 128);
        for i in (0..count).rev() {
            ensure!(Instant::now() < deadline, "Accessibility request expired");
            if let Some(child) = object_call(|error| unsafe {
                ffi::atspi_accessible_get_child_at_index(node.ptr(), i, error)
            })? {
                pending.push((child, depth + 1));
            }
        }
    }
    Ok(None)
}

struct Object(NonNull<c_void>);

impl Object {
    fn from_full(pointer: *mut c_void) -> Option<Self> {
        NonNull::new(pointer).map(Self)
    }
    fn ptr(&self) -> *mut c_void {
        self.0.as_ptr()
    }
    fn refresh(&self) {
        unsafe {
            ffi::atspi_accessible_clear_cache_single(self.ptr());
        }
    }
    fn has_state(&self, state: c_uint) -> Result<bool> {
        let states = Object::from_full(unsafe { ffi::atspi_accessible_get_state_set(self.ptr()) })
            .ok_or_else(|| anyhow!("Accessible state is unavailable"))?;
        Ok(unsafe { ffi::atspi_state_set_contains(states.ptr(), state) } != 0)
    }
    fn is_secure(&self) -> Result<bool> {
        Ok(
            checked(|error| unsafe { ffi::atspi_accessible_get_role(self.ptr(), error) })?
                == ffi::ROLE_PASSWORD_TEXT,
        )
    }
    fn identity(&self, pid: u32) -> Result<String> {
        // These are public ABI prefixes from atspi-object.h and
        // atspi-application.h. Bus unique name + object path identifies the
        // field; the accessible label or GtkBuilder ID does not.
        let base = unsafe { &*(self.ptr().cast::<ffi::AtspiObjectPrefix>()) };
        ensure!(
            !base.app.is_null() && !base.path.is_null(),
            "Accessible identity is unavailable"
        );
        let application = unsafe { &*base.app };
        ensure!(
            !application.bus_name.is_null(),
            "Accessible bus identity is unavailable"
        );
        let bus = unsafe { CStr::from_ptr(application.bus_name) }.to_str()?;
        let path = unsafe { CStr::from_ptr(base.path) }.to_str()?;
        ensure!(
            bus.starts_with(':') && path.starts_with('/'),
            "Accessible identity is invalid"
        );
        Ok(format!("{pid}:{bus}:{path}"))
    }
    fn snapshot(&self, pid: u32, deadline: Instant) -> Result<FieldSnapshot> {
        ensure!(Instant::now() < deadline, "Accessibility request expired");
        self.refresh();
        ensure!(
            checked(|error| unsafe { ffi::atspi_accessible_get_process_id(self.ptr(), error) })?
                == pid,
            "The application process changed"
        );
        ensure!(
            self.has_state(ffi::STATE_FOCUSED)?,
            "The field is no longer focused"
        );
        // Never request Text or EditableText before checking the secure role.
        ensure!(!self.is_secure()?, "Password field; assistance disabled");
        ensure!(
            self.has_state(ffi::STATE_EDITABLE)? && !self.has_state(ffi::STATE_READ_ONLY)?,
            "The field is not editable"
        );
        let text = Object::from_full(unsafe { ffi::atspi_accessible_get_text_iface(self.ptr()) })
            .ok_or_else(|| anyhow!("The field has no accessible text"))?;
        let count =
            checked(|error| unsafe { ffi::atspi_text_get_character_count(text.ptr(), error) })?;
        ensure!(
            (0..=MAX_FIELD_CHARACTERS as c_int).contains(&count),
            "The field is too large for verified replacement"
        );
        let caret =
            checked(|error| unsafe { ffi::atspi_text_get_caret_offset(text.ptr(), error) })?;
        ensure!((0..=count).contains(&caret), "The caret is invalid");
        ensure!(
            checked(|error| unsafe { ffi::atspi_text_get_n_selections(text.ptr(), error) })? == 0,
            "Text is selected; use Copy"
        );
        let contents =
            string_call(|error| unsafe { ffi::atspi_text_get_text(text.ptr(), 0, count, error) })?;
        ensure!(
            contents.chars().count() == count as usize,
            "Accessible text length changed"
        );
        ensure!(
            checked(|error| unsafe { ffi::atspi_text_get_character_count(text.ptr(), error) })?
                == count
                && checked(|error| unsafe { ffi::atspi_text_get_caret_offset(text.ptr(), error) })?
                    == caret,
            "Accessible text changed during inspection"
        );
        let mut error = ptr::null_mut();
        let visible = self.has_state(ffi::STATE_VISIBLE).unwrap_or(false)
            && self.has_state(ffi::STATE_SHOWING).unwrap_or(false);
        let rectangle = if visible {
            unsafe { ffi::atspi_text_get_character_extents(text.ptr(), caret, 0, &mut error) }
        } else {
            ptr::null_mut()
        };
        let rect = if !rectangle.is_null() && error.is_null() {
            let rect = unsafe { *rectangle };
            (rect.height > 0 && rect.width >= 0 && rect.width <= 10000 && rect.height <= 10000)
                .then_some(rect)
        } else {
            None
        };
        unsafe {
            if !rectangle.is_null() {
                glib_ffi::g_free(rectangle.cast());
            }
            if !error.is_null() {
                glib_ffi::g_error_free(error);
            }
        }
        Ok(FieldSnapshot {
            identity: self.identity(pid)?,
            text: contents,
            caret: caret as usize,
            rect,
        })
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            gobject_ffi::g_object_unref(self.ptr().cast());
        }
    }
}

fn checked<T>(call: impl FnOnce(*mut *mut glib_ffi::GError) -> T) -> Result<T> {
    let mut error = ptr::null_mut();
    let result = call(&mut error);
    if error.is_null() {
        Ok(result)
    } else {
        unsafe {
            glib_ffi::g_error_free(error);
        }
        Err(anyhow!("Accessibility call failed"))
    }
}

fn object_call(
    call: impl FnOnce(*mut *mut glib_ffi::GError) -> *mut c_void,
) -> Result<Option<Object>> {
    let mut error = ptr::null_mut();
    let object = Object::from_full(call(&mut error));
    if error.is_null() {
        Ok(object)
    } else {
        unsafe {
            glib_ffi::g_error_free(error);
        }
        Err(anyhow!("Accessibility object is unavailable"))
    }
}

fn string_call(call: impl FnOnce(*mut *mut glib_ffi::GError) -> *mut c_char) -> Result<String> {
    let mut error = ptr::null_mut();
    let text = call(&mut error);
    let result = if error.is_null() && !text.is_null() {
        let bytes = unsafe { CStr::from_ptr(text) };
        if bytes.to_bytes().len() <= MAX_FIELD_CHARACTERS * 4 {
            bytes
                .to_str()
                .map(String::from)
                .map_err(|_| anyhow!("Accessible text is not valid UTF-8"))
        } else {
            Err(anyhow!("Accessible text exceeds the inspection limit"))
        }
    } else {
        Err(anyhow!("Accessible text is unavailable"))
    };
    unsafe {
        if !text.is_null() {
            glib_ffi::g_free(text.cast());
        }
        if !error.is_null() {
            glib_ffi::g_error_free(error);
        }
    }
    result
}

mod ffi {
    use super::*;
    // Values from the stable enums in atspi-constants.h.
    pub const ROLE_PASSWORD_TEXT: c_uint = 40;
    pub const STATE_EDITABLE: c_uint = 7;
    pub const STATE_FOCUSED: c_uint = 12;
    pub const STATE_SHOWING: c_uint = 25;
    pub const STATE_VISIBLE: c_uint = 30;
    pub const STATE_READ_ONLY: c_uint = 43;

    #[repr(C)]
    pub struct AtspiObjectPrefix {
        pub parent: gobject_ffi::GObject,
        pub app: *mut AtspiApplicationPrefix,
        pub path: *mut c_char,
    }
    #[repr(C)]
    pub struct AtspiApplicationPrefix {
        pub parent: gobject_ffi::GObject,
        pub hash: *mut c_void,
        pub bus_name: *mut c_char,
    }

    #[link(name = "atspi")]
    unsafe extern "C" {
        pub fn atspi_init() -> c_int;
        pub fn atspi_exit() -> c_int;
        pub fn atspi_set_timeout(timeout: c_int, startup: c_int);
        pub fn atspi_set_main_context(context: *mut glib_ffi::GMainContext);
        pub fn atspi_get_desktop(index: c_int) -> *mut c_void;
        pub fn atspi_accessible_get_process_id(
            object: *mut c_void,
            error: *mut *mut glib_ffi::GError,
        ) -> c_uint;
        pub fn atspi_accessible_get_child_count(
            object: *mut c_void,
            error: *mut *mut glib_ffi::GError,
        ) -> c_int;
        pub fn atspi_accessible_get_child_at_index(
            object: *mut c_void,
            index: c_int,
            error: *mut *mut glib_ffi::GError,
        ) -> *mut c_void;
        pub fn atspi_accessible_get_role(
            object: *mut c_void,
            error: *mut *mut glib_ffi::GError,
        ) -> c_uint;
        pub fn atspi_accessible_get_state_set(object: *mut c_void) -> *mut c_void;
        pub fn atspi_accessible_set_cache_mask(object: *mut c_void, mask: c_uint);
        pub fn atspi_accessible_clear_cache_single(object: *mut c_void);
        pub fn atspi_state_set_contains(states: *mut c_void, state: c_uint) -> c_int;
        pub fn atspi_accessible_get_text_iface(object: *mut c_void) -> *mut c_void;
        pub fn atspi_accessible_get_editable_text_iface(object: *mut c_void) -> *mut c_void;
        pub fn atspi_text_get_character_count(
            object: *mut c_void,
            error: *mut *mut glib_ffi::GError,
        ) -> c_int;
        pub fn atspi_text_get_caret_offset(
            object: *mut c_void,
            error: *mut *mut glib_ffi::GError,
        ) -> c_int;
        pub fn atspi_text_get_n_selections(
            object: *mut c_void,
            error: *mut *mut glib_ffi::GError,
        ) -> c_int;
        pub fn atspi_text_get_text(
            object: *mut c_void,
            start: c_int,
            end: c_int,
            error: *mut *mut glib_ffi::GError,
        ) -> *mut c_char;
        pub fn atspi_text_get_character_extents(
            object: *mut c_void,
            offset: c_int,
            coordinates: c_uint,
            error: *mut *mut glib_ffi::GError,
        ) -> *mut Rect;
        pub fn atspi_editable_text_set_text_contents(
            object: *mut c_void,
            contents: *const c_char,
            error: *mut *mut glib_ffi::GError,
        ) -> c_int;
    }
}

fn verified_replacement(
    expected: &FieldSnapshot,
    current: &FieldSnapshot,
    typed: &str,
    start: usize,
    end: usize,
    replacement: &str,
) -> Option<String> {
    if expected.identity != current.identity
        || expected.text != current.text
        || expected.caret != current.caret
        || typed.is_empty()
        || start > end
    {
        return None;
    }
    let full: Vec<char> = current.text.chars().collect();
    let segment: Vec<char> = typed.chars().collect();
    if full.len() > MAX_FIELD_CHARACTERS
        || current.caret > full.len()
        || segment.len() > current.caret
        || end > segment.len()
    {
        return None;
    }
    let base = current.caret - segment.len();
    if full[base..current.caret] != segment {
        return None;
    }
    let result: String = full[..base + start]
        .iter()
        .copied()
        .chain(replacement.chars())
        .chain(full[base + end..].iter().copied())
        .collect();
    (result.chars().count() <= MAX_FIELD_CHARACTERS).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(identity: &str, text: &str, caret: usize) -> FieldSnapshot {
        FieldSnapshot {
            identity: identity.into(),
            text: text.into(),
            caret,
            rect: None,
        }
    }

    #[test]
    fn replaces_only_the_typed_span_with_mixed_unicode() {
        let current = field("7::1.12:/field/1", "قبل: I recieved it. بعد", 19);
        let result = verified_replacement(&current, &current, "I recieved it.", 2, 10, "received");
        assert_eq!(result.as_deref(), Some("قبل: I received it. بعد"));
    }

    #[test]
    fn rejects_same_text_in_a_different_field_or_process() {
        let expected = field("7::1.12:/field/1", "hello", 5);
        for identity in ["7::1.12:/field/2", "8::1.13:/field/1"] {
            let current = field(identity, "hello", 5);
            assert!(verified_replacement(&expected, &current, "hello", 0, 5, "hi").is_none());
        }
    }

    #[test]
    fn rejects_changed_surrounding_text_or_caret() {
        let expected = field("field", "before hello after", 12);
        for current in [
            field("field", "BEFORE hello after", 12),
            field("field", "before hello after", 11),
        ] {
            assert!(verified_replacement(&expected, &current, "hello", 0, 5, "hi").is_none());
        }
    }

    #[test]
    fn rejects_a_segment_that_is_not_immediately_before_the_caret() {
        let current = field("field", "hello world", 11);
        assert!(verified_replacement(&current, &current, "hello", 0, 5, "hi").is_none());
    }

    #[test]
    fn validates_unicode_offsets_and_size_before_producing_an_edit() {
        let current = field("field", "هذا خطأ", 7);
        assert_eq!(
            verified_replacement(&current, &current, "هذا خطأ", 4, 7, "صحيح").as_deref(),
            Some("هذا صحيح")
        );
        assert!(verified_replacement(&current, &current, "هذا خطأ", 7, 4, "x").is_none());
        assert!(verified_replacement(&current, &current, "هذا خطأ", 0, 8, "x").is_none());
        assert!(verified_replacement(&current, &current, "", 0, 0, "x").is_none());
        assert!(
            verified_replacement(
                &current,
                &current,
                "هذا خطأ",
                0,
                7,
                &"x".repeat(MAX_FIELD_CHARACTERS + 1)
            )
            .is_none()
        );
    }
}
