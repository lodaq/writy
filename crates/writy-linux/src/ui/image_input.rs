//! Local image input. Child anchors keep pending results attached to their paste location.
use super::*;
use anyhow::{Context, Result, ensure};
use std::{collections::VecDeque, ffi::OsStr};

const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PIXELS: i64 = 32_000_000;

fn recognized_text(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes)?.trim();
    ensure!(!text.is_empty(), "No text found");
    ensure!(
        text.chars().count() <= 8192,
        "Extracted text exceeds 8192 characters"
    );
    Ok(text.to_owned())
}

async fn recognize(bytes: &glib::Bytes) -> Result<String> {
    let languages = process(&["tesseract", "--list-langs"], None, 5).await?;
    let languages = std::str::from_utf8(languages.as_deref().unwrap_or_default())?;
    ensure!(
        ["eng", "ara"]
            .iter()
            .all(|language| languages.lines().any(|line| line.trim() == *language)),
        "Install Tesseract English and Arabic language data (tesseract-data-eng and tesseract-data-ara)"
    );
    let out = process(
        &["tesseract", "stdin", "stdout", "-l", "eng+ara"],
        Some(bytes),
        30,
    )
    .await?;
    recognized_text(out.as_deref().unwrap_or_default())
}

async fn process(
    args: &[&str],
    input: Option<&glib::Bytes>,
    seconds: u64,
) -> Result<Option<glib::Bytes>> {
    let mut flags = gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE;
    if input.is_some() {
        flags |= gio::SubprocessFlags::STDIN_PIPE;
    }
    let argv: Vec<_> = args.iter().map(OsStr::new).collect();
    let child = gio::Subprocess::newv(&argv, flags)
        .with_context(|| format!("Cannot start {}; check that it is installed", args[0]))?;
    let stderr =
        glib::MainContext::default().spawn_local(read_output(child.stderr_pipe().unwrap(), 16_384));
    let writer = input.map(|bytes| {
        let bytes = bytes.clone();
        let pipe = child.stdin_pipe().unwrap();
        glib::MainContext::default().spawn_local(async move {
            let (_, _, error) = pipe
                .write_all_future(bytes, glib::Priority::DEFAULT)
                .await
                .map_err(|(_, error)| error)?;
            if let Some(error) = error {
                return Err(error);
            }
            pipe.close_future(glib::Priority::DEFAULT).await
        })
    });
    let result = glib::future_with_timeout(Duration::from_secs(seconds), async {
        let out = read_output(child.stdout_pipe().unwrap(), MAX_IMAGE_BYTES).await?;
        let err = stderr.await.map_err(|e| anyhow::anyhow!("{e}"))??;
        if let Some(writer) = writer {
            writer.await.map_err(|e| anyhow::anyhow!("{e}"))??;
        }
        child.wait_future().await?;
        Ok::<_, anyhow::Error>((out, err))
    })
    .await;
    let (out, err) = match result {
        Ok(Ok(result)) => result,
        result => {
            child.force_exit();
            match result {
                Ok(Err(error)) => return Err(error),
                _ => anyhow::bail!("{} timed out", args[0]),
            }
        }
    };
    if args == ["slurp"] && child.has_exited() && child.exit_status() == 1 {
        return Ok(None);
    }
    ensure!(
        child.is_successful(),
        "{} failed: {}",
        args[0],
        String::from_utf8_lossy(&err)
            .chars()
            .take(400)
            .collect::<String>()
    );
    Ok(Some(out))
}

async fn read_output(stream: gio::InputStream, limit: usize) -> Result<glib::Bytes> {
    let mut bytes = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(
                (limit + 1 - bytes.len()).min(65_536),
                glib::Priority::DEFAULT,
            )
            .await?;
        if chunk.is_empty() {
            return Ok(glib::Bytes::from_owned(bytes));
        }
        bytes.extend_from_slice(&chunk);
        ensure!(
            bytes.len() <= limit,
            "Image or process output exceeds its size limit"
        );
    }
}

#[derive(Clone)]
struct Job {
    anchor: gtk::TextChildAnchor,
    row: gtk::Box,
    bytes: Rc<RefCell<Option<Result<glib::Bytes>>>>,
}

#[derive(Clone)]
pub(super) struct ImageInput {
    editor: glib::WeakRef<gtk::TextView>,
    queue: Rc<RefCell<VecDeque<Job>>>,
    running: Rc<RefCell<bool>>,
    capturing: Rc<RefCell<bool>>,
}

impl ImageInput {
    pub fn new(editor: &gtk::TextView) -> Self {
        let this = Self {
            editor: editor.downgrade(),
            queue: Default::default(),
            running: Default::default(),
            capturing: Default::default(),
        };
        let input = this.clone();
        editor.connect_paste_clipboard(move |editor| {
            let clipboard = editor.clipboard();
            if !clipboard
                .formats()
                .contains_type(gdk::Texture::static_type())
                && !clipboard
                    .formats()
                    .mime_types()
                    .iter()
                    .any(|m| m.starts_with("image/"))
            {
                return;
            }
            editor.stop_signal_emission_by_name("paste-clipboard");
            let job = input.badge(editor);
            input.queue.borrow_mut().push_back(job.clone());
            input.read_clipboard(job, clipboard);
        });
        this
    }

    fn read_clipboard(&self, job: Job, clipboard: gdk::Clipboard) {
        let input = self.clone();
        glib::MainContext::default().spawn_local(async move {
            *job.bytes.borrow_mut() = Some(
                async {
                    let texture = glib::future_with_timeout(
                        Duration::from_secs(5),
                        clipboard.read_texture_future(),
                    )
                    .await
                    .context("Clipboard image read timed out")??
                    .context("Clipboard image is unavailable")?;
                    ensure!(
                        i64::from(texture.width()) * i64::from(texture.height()) <= MAX_PIXELS,
                        "Image exceeds 32 million pixels"
                    );
                    let bytes = texture.save_to_png_bytes();
                    ensure!(bytes.len() <= MAX_IMAGE_BYTES, "Image exceeds 16 MiB");
                    Ok(bytes)
                }
                .await,
            );
            input.start_next();
        });
    }

    fn badge(&self, editor: &gtk::TextView) -> Job {
        let buffer = editor.buffer();
        buffer.begin_user_action();
        buffer.delete_selection(true, true);
        let mut iter = buffer.iter_at_offset(buffer.cursor_position());
        let anchor = buffer.create_child_anchor(&mut iter);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.add_css_class("writy-image-badge");
        row.update_property(&[gtk::accessible::Property::Label("Extracting image text")]);
        editor.add_child_at_anchor(&row, &anchor);
        buffer.end_user_action();
        loading(&row);
        Job {
            anchor,
            row,
            bytes: Default::default(),
        }
    }

    fn insert(&self, bytes: glib::Bytes) {
        if let Some(editor) = self.editor.upgrade() {
            let job = self.badge(&editor);
            *job.bytes.borrow_mut() = Some(Ok(bytes));
            self.queue.borrow_mut().push_back(job);
            self.start_next();
        }
    }

    fn start_next(&self) {
        if *self.running.borrow() {
            return;
        }
        loop {
            let mut queue = self.queue.borrow_mut();
            let Some(job) = queue.front() else {
                return;
            };
            if job.anchor.is_deleted() {
                queue.pop_front();
                continue;
            }
            if job.bytes.borrow().is_none() {
                return;
            }
            let job = queue.pop_front().unwrap();
            drop(queue);
            *self.running.borrow_mut() = true;
            let input = self.clone();
            glib::MainContext::default().spawn_local(async move {
                let data = job.bytes.borrow_mut().take().unwrap();
                let result = match &data {
                    Ok(bytes) => recognize(bytes).await,
                    Err(error) => Err(anyhow::anyhow!("{error:#}")),
                };
                if !job.anchor.is_deleted() {
                    match result {
                        Ok(text) => {
                            if let Some(editor) = input.editor.upgrade() {
                                let buffer = editor.buffer();
                                buffer.begin_user_action();
                                let mut start = buffer.iter_at_child_anchor(&job.anchor);
                                let mut end = start;
                                end.forward_char();
                                buffer.delete(&mut start, &mut end);
                                buffer.insert(&mut start, &text);
                                buffer.end_user_action();
                            }
                        }
                        Err(error) => input.failed(&job, data.ok(), &format!("{error:#}")),
                    }
                }
                *input.running.borrow_mut() = false;
                input.start_next();
            });
            return;
        }
    }

    fn failed(&self, job: &Job, bytes: Option<glib::Bytes>, reason: &str) {
        clear_row(&job.row);
        let title = if reason == "No text found" {
            reason
        } else {
            "Extraction failed"
        };
        job.row.append(&gtk::Label::new(Some(title)));
        job.row.set_tooltip_text(Some(reason));
        job.row
            .update_property(&[gtk::accessible::Property::Label(title)]);
        let retry = gtk::Button::with_label("Retry");
        let remove = gtk::Button::with_label("Remove");
        job.row.append(&retry);
        job.row.append(&remove);
        let input = self.clone();
        let anchor = job.anchor.clone();
        let row = job.row.downgrade();
        retry.connect_clicked(move |_| {
            if anchor.is_deleted() {
                return;
            }
            let Some(row) = row.upgrade() else {
                return;
            };
            loading(&row);
            let job = Job {
                anchor: anchor.clone(),
                row,
                bytes: Default::default(),
            };
            input.queue.borrow_mut().push_back(job.clone());
            if let Some(bytes) = &bytes {
                *job.bytes.borrow_mut() = Some(Ok(bytes.clone()));
                input.start_next();
            } else {
                // A failed clipboard read has no retained image; retry the current clipboard.
                if let Some(editor) = input.editor.upgrade() {
                    input.read_clipboard(job, editor.clipboard());
                }
            }
        });
        let editor = self.editor.clone();
        let anchor = job.anchor.clone();
        remove.connect_clicked(move |_| {
            if !anchor.is_deleted()
                && let Some(editor) = editor.upgrade()
            {
                let buffer = editor.buffer();
                let mut start = buffer.iter_at_child_anchor(&anchor);
                let mut end = start;
                end.forward_char();
                buffer.delete(&mut start, &mut end);
            }
        });
    }

    pub fn is_capturing(&self) -> bool {
        *self.capturing.borrow()
    }

    pub fn capture(
        &self,
        search: &gtk::ApplicationWindow,
        hud: &gtk::ApplicationWindow,
        tx: &Commands,
        status: &gtk::Label,
    ) {
        if *self.capturing.borrow() {
            return;
        }
        *self.capturing.borrow_mut() = true;
        let was_visible = search.is_visible();
        let hud_visible = hud.is_visible();
        search.set_visible(false);
        hud.set_visible(false);
        let (search, hud, tx, status) = (search.clone(), hud.clone(), tx.clone(), status.clone());
        let input = self.clone();
        glib::MainContext::default().spawn_local(async move {
            glib::timeout_future(Duration::from_millis(150)).await;
            let result: Result<Option<glib::Bytes>> = async {
                let selection = process(&["slurp"], None, 120).await?;
                let geometry =
                    std::str::from_utf8(selection.as_deref().unwrap_or_default())?.trim();
                if geometry.is_empty() {
                    return Ok(None);
                }
                let bytes = process(&["grim", "-g", geometry, "-"], None, 15)
                    .await?
                    .context("Screenshot is empty")?;
                let texture = gdk::Texture::from_bytes(&bytes)?;
                ensure!(
                    i64::from(texture.width()) * i64::from(texture.height()) <= MAX_PIXELS,
                    "Image exceeds 32 million pixels"
                );
                Ok(Some(bytes))
            }
            .await;
            match result {
                Ok(Some(bytes)) => {
                    if let Some(editor) = input.editor.upgrade() {
                        editor.buffer().set_text("");
                        send(
                            &tx,
                            Command::Query {
                                text: String::new(),
                                translate: false,
                            },
                        );
                        input.insert(bytes);
                        search.present();
                        editor.grab_focus();
                    }
                }
                result => {
                    if was_visible {
                        search.present();
                    }
                    if hud_visible {
                        hud.set_visible(true);
                    }
                    if let Err(error) = result {
                        status.set_text(&format!("Capture unavailable: {error:#}"));
                        eprintln!("Writy: capture unavailable: {error:#}");
                        if !was_visible {
                            gtk::AlertDialog::builder()
                                .message("Screenshot unavailable")
                                .detail(format!("{error:#}"))
                                .build()
                                .show(Some(&search));
                        }
                    }
                }
            }
            *input.capturing.borrow_mut() = false;
        });
    }
}

fn clear_row(row: &gtk::Box) {
    while let Some(child) = row.first_child() {
        row.remove(&child);
    }
}
fn loading(row: &gtk::Box) {
    clear_row(row);
    row.update_property(&[gtk::accessible::Property::Label("Extracting image text")]);
    row.set_tooltip_text(None);
    let spinner = gtk::Spinner::new();
    spinner.start();
    row.append(&spinner);
    row.append(&gtk::Label::new(Some("Extracting text…")));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for(mut done: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let context = glib::MainContext::default();
        while !done() {
            assert!(
                std::time::Instant::now() < deadline,
                "image input timed out"
            );
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    #[ignore = "requires isolated GTK display, Tesseract eng/ara, and stub capture tools; run tests/desktop/image_input.py"]
    fn image_input_runtime() {
        gtk::init().unwrap();
        glib::MainContext::default().block_on(async {
            let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_static(b"12345"));
            assert!(read_output(stream.upcast(), 4).await.is_err());
            let error = process(&["/bin/sh", "-c", "printf diagnostic >&2; exit 7"], None, 2)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("diagnostic"));
            assert!(
                process(&["/bin/sleep", "2"], None, 1)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("timed out")
            );
        });
        let editor = gtk::TextView::new();
        let input = ImageInput::new(&editor);
        let buffer = editor.buffer();
        let text = || {
            buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string()
        };
        let png =
            glib::Bytes::from_static(include_bytes!("../../../../tests/fixtures/ocr-english.png"));
        let texture = gdk::Texture::from_bytes(&png).unwrap();
        let display = gdk::Display::default().unwrap();
        display.clipboard().set_text("plain text");
        editor.emit_by_name::<()>("paste-clipboard", &[]);
        wait_for(|| text() == "plain text");
        display.clipboard().set_texture(&texture);
        buffer.set_text("before  after");
        buffer.place_cursor(&buffer.iter_at_offset(7));
        editor.emit_by_name::<()>("paste-clipboard", &[]);
        assert_eq!(
            text(),
            "before  after",
            "lookup text must exclude the badge"
        );
        assert_eq!(buffer.char_count(), 14, "one child anchor inserted");
        buffer.insert(&mut buffer.start_iter(), "Edit ");
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        assert_eq!(text(), "Edit before Hello world after");

        buffer.set_text("");
        input.insert(glib::Bytes::from_static(include_bytes!(
            "../../../../tests/fixtures/ocr-mixed.png"
        )));
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        assert!(text().contains("Hello world") && text().contains("مرحبا بالعالم"));

        // Selection replacement, and two queued images inserted in paste order.
        buffer.set_text("left replace right");
        buffer.select_range(&buffer.iter_at_offset(5), &buffer.iter_at_offset(12));
        input.insert(png.clone());
        buffer.place_cursor(&buffer.end_iter());
        input.insert(png.clone());
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        assert_eq!(text(), "left Hello world rightHello world");

        // Deleting an in-flight anchor prevents stale insertion into new input.
        input.insert(png.clone());
        buffer.set_text("Keep this");
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        assert_eq!(text(), "Keep this");

        // Invalid images leave a retry/remove badge and preserve surrounding text.
        input.insert(glib::Bytes::from_static(b"invalid image"));
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        let anchor = buffer
            .iter_at_offset(buffer.char_count() - 1)
            .child_anchor()
            .unwrap();
        let row = anchor.widgets()[0].clone().downcast::<gtk::Box>().unwrap();
        assert_eq!(
            row.first_child()
                .unwrap()
                .downcast::<gtk::Label>()
                .unwrap()
                .text(),
            "Extraction failed"
        );
        row.first_child()
            .unwrap()
            .next_sibling()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap()
            .emit_clicked();
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        row.last_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap()
            .emit_clicked();
        assert_eq!(text(), "Keep this");

        // No text is distinct from invalid data; Retry retains the original PNG.
        let blank = gdk::MemoryTexture::new(
            100,
            100,
            gdk::MemoryFormat::R8g8b8,
            &glib::Bytes::from_owned(vec![255u8; 30000]),
            300,
        );
        input.insert(blank.save_to_png_bytes());
        wait_for(|| !*input.running.borrow() && input.queue.borrow().is_empty());
        let anchor = buffer
            .iter_at_offset(buffer.char_count() - 1)
            .child_anchor()
            .unwrap();
        let row = anchor.widgets()[0].clone().downcast::<gtk::Box>().unwrap();
        assert_eq!(
            row.first_child()
                .unwrap()
                .downcast::<gtk::Label>()
                .unwrap()
                .text(),
            "No text found"
        );
        buffer.set_text("Existing input");

        let app = gtk::Application::builder()
            .application_id("io.github.lodaq.writy.image-test")
            .build();
        app.register(None::<&gio::Cancellable>).unwrap();
        let search = gtk::ApplicationWindow::builder()
            .application(&app)
            .child(&editor)
            .build();
        let hud = gtk::ApplicationWindow::builder().application(&app).build();
        let status = gtk::Label::new(None);
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        input.capture(&search, &hud, &tx, &status);
        wait_for(|| !*input.capturing.borrow());
        assert_eq!(text(), "Existing input", "first stub selection cancels");
        assert!(!search.is_visible());
        search.present();
        input.capture(&search, &hud, &tx, &status);
        wait_for(|| {
            !*input.capturing.borrow()
                && !*input.running.borrow()
                && input.queue.borrow().is_empty()
        });
        assert_eq!(text(), "Hello world", "completed capture starts fresh");
        assert!(search.is_visible());
        assert!(matches!(rx.try_recv().unwrap().0, Command::Query { text, .. } if text.is_empty()));
        search.close();
        hud.close();
    }

    #[test]
    fn extracted_text_is_bounded_without_losing_arabic() {
        assert_eq!(
            recognized_text(" hello مرحبا \n".as_bytes()).unwrap(),
            "hello مرحبا"
        );
        assert!(recognized_text(b" \n").is_err());
        assert!(recognized_text("م".repeat(8193).as_bytes()).is_err());
        assert!(recognized_text(&[0xff]).is_err());
    }
}
