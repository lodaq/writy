use crate::{
    accessibility::{Accessibility, FieldSnapshot, Observation},
    input::{Decoded, Decoder},
    ipc,
    protocol::{BridgeEvent, Command, Results, UiEvent},
};
use anyhow::{Context, Result};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::SyncSender,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::{mpsc, oneshot},
};
use writy_core::{
    buffer::TypingBuffer,
    config::Config,
    dictionary::Dictionary,
    linter::EnglishChecker,
    provider::{ModelService, WritingAssistance, writing_word},
    types::{
        Attribution, Capability, Confidence, LexicalEntry, ProviderResult, Sense, Suggestion,
        SuggestionKind, TextSnapshot,
    },
};

pub type Commands = mpsc::Sender<(Command, Option<oneshot::Sender<String>>)>;
enum Event {
    Connected(u64, mpsc::Sender<serde_json::Value>),
    Disconnected(u64),
    Bridge(u64, BridgeEvent),
    Model(u64, Result<ProviderResult>),
    Checked(TextSnapshot, Vec<Suggestion>),
    CheckerReady,
    Observation {
        epoch: u64,
        revision: Option<u64>,
        observation: Observation,
    },
}
pub fn start(config: Config, ui: SyncSender<UiEvent>, generation: Arc<AtomicU64>) -> Commands {
    let (tx, rx) = mpsc::channel(64);
    std::thread::Builder::new()
        .name("writy-services".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            if let Err(e) = runtime.block_on(run(config, ui.clone(), generation, rx)) {
                eprintln!("Writy services stopped: {e}");
                let _ = ui.try_send(UiEvent::Notice(format!("Services stopped: {e}")));
            }
        })
        .expect("service thread");
    tx
}
struct State {
    config: Config,
    ui: SyncSender<UiEvent>,
    generation: Arc<AtomicU64>,
    buffer: TypingBuffer,
    decoder: Decoder,
    dictionary: Dictionary,
    checker: tokio::sync::watch::Sender<Option<TextSnapshot>>,
    access: Accessibility,
    current: Option<TextSnapshot>,
    anchor: Option<(u64, FieldSnapshot)>,
    suggestions: Vec<Suggestion>,
    entries: Vec<writy_core::types::LexicalEntry>,
    translation: Option<(String, String)>,
    notice: String,
    app: String,
    window: String,
    pid: u32,
    epoch: u64,
    seq: u64,
    prose: bool,
    secure: bool,
    locked: bool,
    bridge: Option<mpsc::Sender<serde_json::Value>>,
    model: Option<tokio::task::JoinHandle<()>>,
    last_change: Instant,
    local_revision: u64,
    model_revision: u64,
    search: bool,
    hud_interacting: bool,
    selection: bool,
    selection_position: Option<(i32, i32)>,
    inspecting: bool,
    access_status: String,
    checker_ready: bool,
    bridge_session: u64,
    output: Option<String>,
    output_bounds: Option<(i32, i32, i32, i32)>,
}
fn screenshot_app(app: &str) -> bool {
    app.eq_ignore_ascii_case("flameshot") || app.eq_ignore_ascii_case("org.flameshot.Flameshot")
}
impl State {
    fn clear(&mut self) {
        self.clear_with_huds(false);
    }
    fn clear_with_huds(&mut self, preserve_huds: bool) {
        self.buffer.reset();
        self.decoder.reset();
        self.current = None;
        self.selection_position = None;
        self.anchor = None;
        self.suggestions.clear();
        self.entries.clear();
        self.translation = None;
        self.notice.clear();
        self.local_revision = u64::MAX;
        self.model_revision = u64::MAX;
        if let Some(task) = self.model.take() {
            task.abort();
        }
        let g = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.ui.try_send(UiEvent::Clear {
            generation: g,
            preserve_huds,
        });
    }
    fn invalidate_capture(&mut self, preserve_huds: bool) {
        if self.search {
            // Explicit search has its own input. Compositor clicks and keyboard
            // resets invalidate ambient editing anchors, not the search query.
            self.buffer.reset();
            self.decoder.reset();
            self.anchor = None;
        } else {
            self.clear_with_huds(preserve_huds || self.hud_interacting);
        }
    }
    fn allowed(&self) -> bool {
        self.config
            .capture_allowed(&self.app, self.prose, self.secure)
            && !self.search
            && !screenshot_app(&self.app)
            && !self.locked
    }
    fn capture_lease(&self) {
        if let Some(tx) = &self.bridge {
            let _=tx.try_send(serde_json::json!({"type":"capture","enabled":self.allowed(),"focus_epoch":self.epoch}));
        }
    }
    fn publish(&self) {
        let Some(snapshot) = self.current.clone() else {
            return;
        };
        let caret = self
            .selection_position
            .map(|(x, y)| (x, y, 0, 0))
            .or_else(|| {
                self.anchor
                    .as_ref()
                    .and_then(|(_, f)| f.rect.map(|r| (r.x, r.y, r.width, r.height)))
            });
        let _ = self.ui.try_send(UiEvent::Results(Box::new(Results {
            writing: !self.search && !self.selection,
            output_bounds: self.output_bounds,
            generation: self.generation.load(Ordering::SeqCst),
            snapshot,
            entries: self.entries.clone(),
            suggestions: self.suggestions.clone(),
            translation: self.translation.clone(),
            status: self.notice.clone(),
            caret,
            output: self.output.clone(),
        })));
    }
    fn inspect(&mut self, tx: &mpsc::Sender<Event>) {
        if self.inspecting || self.locked {
            return;
        }
        self.inspecting = true;
        let access = self.access.clone();
        let pid = self.pid;
        let epoch = self.epoch;
        let revision = self.current.as_ref().map(|s| s.revision);
        let tx = tx.clone();
        tokio::spawn(async move {
            let observation = access.inspect(pid).await;
            let _ = tx
                .send(Event::Observation {
                    epoch,
                    revision,
                    observation,
                })
                .await;
        });
    }
    fn observation(&mut self, observation: Observation, revision: Option<u64>) {
        self.access_status = observation.status.clone();
        if observation.secure {
            self.secure = true;
            self.clear();
            self.capture_lease();
            return;
        }
        if observation.field.is_some() {
            self.secure = false;
        }
        if let (Some(current), Some(field)) = (&mut self.current, observation.field) {
            let prefix: String = field.text.chars().take(field.caret).collect();
            if revision == Some(current.revision)
                && !current.text.is_empty()
                && prefix.ends_with(&current.text)
                && !self.search
                && !self.selection
            {
                current.confidence = Confidence::Verified;
                self.anchor = Some((current.revision, field));
                for suggestion in &mut self.suggestions {
                    if suggestion.snapshot.same_context(current) {
                        suggestion.snapshot.confidence = Confidence::Verified;
                    }
                }
            } else if revision == Some(current.revision) {
                self.anchor = None;
            }
        }
        self.capture_lease();
    }
    fn request(&mut self, capability: Capability, tx: &mpsc::Sender<Event>) {
        if let Some(task) = self.model.take() {
            task.abort();
        }
        let Some(snapshot) = self.current.clone() else {
            return;
        };
        let service = match ModelService::new(self.config.clone()) {
            Ok(s) => s,
            Err(_) => {
                self.notice = "Invalid model configuration".into();
                self.publish();
                return;
            }
        };
        self.model_revision = snapshot.revision;
        let generation = self.generation.load(Ordering::SeqCst);
        let tx = tx.clone();
        self.model = Some(tokio::spawn(async move {
            let result = service.generate(snapshot, capability).await;
            let _ = tx.send(Event::Model(generation, result)).await;
        }));
        self.notice = "Generating…".into();
        self.publish();
    }
    async fn query(
        &mut self,
        text: String,
        translate: bool,
        selection: bool,
        tx: &mpsc::Sender<Event>,
    ) {
        self.clear();
        self.selection = selection;
        self.selection_position = if selection {
            cursor_position().await
        } else {
            None
        };
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let revision = self.generation.load(Ordering::SeqCst);
        let snapshot = TextSnapshot {
            source: if selection { "selection" } else { "search" }.into(),
            focus_epoch: self.epoch,
            revision,
            language: writy_core::types::Language::detect(&text),
            text,
            confidence: Confidence::Projected,
        };
        self.entries = self.dictionary.lookup(&snapshot.text).unwrap_or_default();
        self.current = Some(snapshot.clone());
        if !translate && !selection {
            self.checker.send_replace(Some(snapshot.clone()));
        }
        self.notice = if self.entries.is_empty() {
            "No local entry. Install dictionaries in Settings or use model translation.".into()
        } else {
            String::new()
        };
        self.publish();
        if translate || snapshot.text.split_whitespace().count() > 1 || self.entries.is_empty() {
            self.request(
                if translate || selection {
                    Capability::Translate
                } else {
                    Capability::Rewrite
                },
                tx,
            );
        }
    }
    async fn command(&mut self, cmd: Command, tx: &mpsc::Sender<Event>) -> String {
        if let Err(message) = cmd.validate() {
            self.clear();
            let message = message.to_owned();
            let _ = self.ui.try_send(UiEvent::Notice(message.clone()));
            return message;
        }
        match cmd {
            Command::HudInteraction { active } => { self.hud_interacting = active; },
            Command::Status=>return serde_json::json!({"enabled":self.config.enabled,"offline":self.config.offline,"bridge_connected":self.bridge.is_some(),"capture":self.allowed()&&self.bridge.is_some(),"app":self.app,"buffer_chars":self.buffer.snapshot().text.chars().count(),"revision":self.buffer.snapshot().revision,"focus_epoch":self.epoch,"suggestions":self.suggestions.len(),"accessibility":self.access_status,"selection_active":self.selection,"dictionary_matches":self.entries.len(),"checker_ready":self.checker_ready,"verified":self.current.as_ref().is_some_and(|s|s.confidence==Confidence::Verified)}).to_string(),
            Command::Search=>{let query=self.current.as_ref().map(|s|s.text.clone());self.clear();self.search=true;self.capture_lease();let _=self.ui.try_send(UiEvent::Search(query));}
            Command::Settings=>{let _=self.ui.try_send(UiEvent::Settings);}
            Command::Lookup{text}=>{self.search=false;self.query(text,true,true,tx).await;}
            Command::Query{text,translate}=>{self.search=true;self.query(text,translate,false,tx).await;}
            Command::Selection{text}=>{
                if self.hud_interacting||!self.config.selection_popup||screenshot_app(&self.app)||!self.config.capture_allowed(&self.app,true,self.secure){return "ignored".into();}
                if let Some(text)=text {if !self.search {self.query(text,true,true,tx).await;}}else if self.selection {self.clear();self.selection=false;}
            }
            Command::Dismiss=>{self.clear();self.search=false;self.selection=false;self.capture_lease();let _=self.ui.try_send(UiEvent::Dismiss);}
            Command::Pause=>{self.config.enabled= !self.config.enabled;self.clear();self.capture_lease();let _=self.ui.try_send(UiEvent::Notice(if self.config.enabled{"Writy resumed"}else{"Writy paused"}.into()));}
            Command::Prose=>{self.prose= !self.prose;self.clear();self.capture_lease();let _=self.ui.try_send(UiEvent::Notice(if self.prose{"Terminal prose mode enabled until focus changes"}else{"Terminal prose mode disabled"}.into()));}
            Command::Offline{enabled}=>{self.config.offline=enabled;self.clear();}
            Command::Reload=>match Config::load(){Ok(c)=>{self.config=c;self.clear();self.capture_lease();},Err(_)=>return "invalid configuration".into()},
            Command::Copy=>{
                let text=self.suggestions.first().and_then(Suggestion::replaced_text).or_else(||self.translation.as_ref().map(|t|t.0.clone())).or_else(||self.entries.iter().flat_map(|e|&e.senses).flat_map(|s|&s.translations).next().cloned());
                if let Some(text)=text {let _=self.ui.try_send(UiEvent::Copy(text));}
            }
            Command::Accept=>{
                let Some(s)=self.suggestions.first().cloned()else{return "no suggestion".into();};
                let Some(current)=&self.current else{return "no current context".into();};
                if !s.matches(current)||current.confidence!=Confidence::Verified {return "Text range is unverified; use Copy".into();}
                let Some((revision,field))=self.anchor.clone()else{return "Text range is unverified; use Copy".into();};
                if revision!=current.revision{return "Suggestion is stale".into();}
                match self.access.verify_replace(self.pid,s,field).await {
                    Ok(())=>{self.clear();return "applied".into();}
                    Err(_)=>return "Text or focus changed; correction was not applied".into(),
                }
            }
        }
        "ok".into()
    }
    async fn bridge_event(&mut self, event: BridgeEvent, tx: &mpsc::Sender<Event>) {
        let seq = event.seq();
        if self.seq != 0 && seq != self.seq + 1 {
            self.invalidate_capture(false);
        }
        self.seq = seq;
        match event {
            BridgeEvent::Hello { version, .. } => {
                self.seq = seq;
                if version != 1 {
                    self.bridge = None;
                }
                self.invalidate_capture(false);
            }
            BridgeEvent::Focus {
                app_id,
                window_id,
                focus_epoch,
                pid,
                locked,
                monitor,
                monitor_geometry,
                ..
            } => {
                let screenshot = screenshot_app(&app_id);
                let _ = self.ui.try_send(UiEvent::Screenshot(screenshot));
                if self.window != window_id || self.epoch != focus_epoch {
                    if !self.search {
                        // Focus returning to the same editor, HUD interaction and the
                        // screenshot overlay must not erase the pixels being referenced.
                        // Editing anchors are still invalidated by clear_with_huds.
                        self.clear_with_huds(
                            screenshot
                                || self.hud_interacting
                                || window_id.is_empty()
                                || self.window == window_id,
                        );
                        self.search = false;
                    }
                    if self.window != window_id {
                        self.prose = false;
                    }
                    self.selection = false;
                }
                if self.window != window_id {
                    self.secure = false;
                }
                self.app = app_id;
                self.window = window_id;
                self.pid = pid;
                let resized = self.output_bounds != monitor_geometry;
                self.output_bounds = monitor_geometry;
                self.output = (!monitor.is_empty()).then_some(monitor);
                if resized {
                    self.publish();
                }
                self.locked = locked;
                if locked {
                    self.clear();
                }
                self.epoch = focus_epoch;
                self.buffer
                    .focus(format!("{}:{}", self.app, self.window), self.epoch);
                self.inspect(tx);
            }
            BridgeEvent::Keymap { device, keymap, .. } => {
                self.decoder.keymap(device, keymap);
                self.invalidate_capture(false);
                self.capture_lease();
            }
            BridgeEvent::Modifiers {
                device,
                depressed,
                latched,
                locked,
                group,
                ..
            } => self
                .decoder
                .modifiers(&device, depressed, latched, locked, group),
            BridgeEvent::Reset {
                focus_epoch,
                reason,
                ..
            } => {
                self.epoch = focus_epoch;
                self.invalidate_capture(matches!(reason.as_str(), "focus" | "window" | "shortcut"));
                self.inspect(tx);
                self.capture_lease();
            }
            BridgeEvent::Key {
                device,
                keycode,
                state,
                focus_epoch,
                ..
            } => {
                if focus_epoch != self.epoch || !self.allowed() {
                    return;
                }
                match self.decoder.key(&device, keycode, state) {
                    Decoded::Text(text) => {
                        if self.selection {
                            self.clear();
                            self.selection = false;
                        }
                        if let Some(task) = self.model.take() {
                            task.abort();
                        }
                        self.buffer.push(&text);
                        self.current = Some(self.buffer.snapshot());
                        self.suggestions.clear();
                        self.entries.clear();
                        self.notice.clear();
                        self.translation = None;
                        self.anchor = None;
                        self.last_change = Instant::now();
                        // A stale visible result is invalidated immediately, even if the UI queue is full.
                        let g = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
                        let _ = self.ui.try_send(UiEvent::Clear {
                            generation: g,
                            preserve_huds: false,
                        });
                    }
                    Decoded::Reset => {
                        self.clear();
                        self.capture_lease();
                    }
                    Decoded::Ignore => {}
                }
            }
        }
    }
}
fn generated_lexical_entry(
    snapshot: &TextSnapshot,
    entries: &[LexicalEntry],
    assistance: &WritingAssistance,
    provider: &str,
) -> LexicalEntry {
    let mut generated = Sense {
        definitions: assistance.definitions.clone(),
        examples: assistance.examples.clone(),
        collocations: assistance.collocations.clone(),
        synonyms: assistance.synonyms.clone(),
        antonyms: assistance.antonyms.clone(),
        word_family: assistance.word_family.clone(),
        ..Sense::default()
    };
    // Dictionary evidence takes precedence category by category.
    for sense in entries.iter().flat_map(|entry| &entry.senses) {
        for (evidence, generated) in [
            (&sense.definitions, &mut generated.definitions),
            (&sense.examples, &mut generated.examples),
            (&sense.collocations, &mut generated.collocations),
            (&sense.synonyms, &mut generated.synonyms),
            (&sense.antonyms, &mut generated.antonyms),
            (&sense.word_family, &mut generated.word_family),
        ] {
            if !evidence.is_empty() {
                generated.clear();
            }
        }
    }
    LexicalEntry {
        headword: writing_word(&snapshot.text).into(),
        language: snapshot.language,
        senses: vec![generated],
        source: Attribution {
            name: format!("AI-generated · {provider}"),
            url: String::new(),
            license: "Model-generated content".into(),
        },
        ..LexicalEntry::default()
    }
}

async fn run(
    config: Config,
    ui: SyncSender<UiEvent>,
    generation: Arc<AtomicU64>,
    mut commands: mpsc::Receiver<(Command, Option<oneshot::Sender<String>>)>,
) -> Result<()> {
    let control = ipc::bind("control.sock").await?;
    let bridge = ipc::bind("bridge.sock").await?;
    let runtime = ipc::runtime_dir()?;
    let _control_cleanup = ipc::SocketCleanup(runtime.join("control.sock"));
    let _bridge_cleanup = ipc::SocketCleanup(runtime.join("bridge.sock"));
    let (events, mut rx) = mpsc::channel(256);
    let (remote_tx, mut remote_rx) = mpsc::channel(64);
    tokio::spawn(control_server(control, remote_tx));
    tokio::spawn(bridge_server(bridge, events.clone()));
    let mut watcher = if config.selection_popup {
        tokio::process::Command::new("wl-paste")
            .args(["--primary", "--watch"])
            .arg(std::env::current_exe()?)
            .arg("selection-event")
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()
    } else {
        None
    };
    let dictionary = Dictionary::open(ipc::database_path())?;
    let mut s = State {
        config,
        ui,
        generation,
        buffer: TypingBuffer::default(),
        decoder: Decoder::default(),
        dictionary,
        checker: start_checker(events.clone()),
        access: Accessibility::start(),
        current: None,
        anchor: None,
        suggestions: vec![],
        entries: vec![],
        translation: None,
        notice: String::new(),
        app: String::new(),
        window: String::new(),
        pid: 0,
        epoch: 0,
        seq: 0,
        prose: false,
        secure: false,
        locked: false,
        bridge: None,
        model: None,
        last_change: Instant::now(),
        local_revision: u64::MAX,
        model_revision: u64::MAX,
        search: false,
        hud_interacting: false,
        selection: false,
        selection_position: None,
        inspecting: false,
        access_status: String::new(),
        checker_ready: false,
        bridge_session: 0,
        output: None,
        output_bounds: None,
    };
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let mut heartbeat = tokio::time::interval(Duration::from_millis(400));
    loop {
        tokio::select! {
            command=commands.recv()=>{let Some((cmd,reply))=command else {break;};let result=s.command(cmd,&events).await;if let Some(reply)=reply{let _=reply.send(result.clone());}
                if result!="ok"&&result!="applied"&&result!="ignored"{let _=s.ui.try_send(UiEvent::Notice(result));}}
            Some((cmd,reply))=remote_rx.recv()=>{let result=s.command(cmd,&events).await;let _=reply.send(result);}
            Some(event)=rx.recv()=>match event {
                Event::Connected(session,tx)=>{s.bridge_session=session;s.bridge=Some(tx);s.seq=0;s.invalidate_capture(false);}
                Event::Disconnected(session)=>{if session==s.bridge_session{s.bridge=None;s.invalidate_capture(false);}}
                Event::Bridge(session,event)=>{if session==s.bridge_session{s.bridge_event(event,&events).await;}}
                Event::Observation{epoch,revision,observation}=>{s.inspecting=false;if epoch==s.epoch{s.observation(observation,revision);}}
                Event::CheckerReady=>{s.checker_ready=true;}
                Event::Checked(snapshot, suggestions)=>{
                    if s.current.as_ref().is_some_and(|c|c.same_context(&snapshot)) {
                        s.suggestions.retain(|s|matches!(s.kind,SuggestionKind::Rewrite|SuggestionKind::Completion));
                        s.suggestions.extend(suggestions);s.publish();
                    }
                }
                Event::Model(g,result)=>{
                    if g!=s.generation.load(Ordering::SeqCst){continue;}
                    match result {
                        Ok(result)=>if s.current.as_ref().is_some_and(|c|c.same_context(&result.snapshot)) {
                            s.notice=result.notice.unwrap_or_default();
                            match result.capability {
                                Capability::WritingAssistance=>match WritingAssistance::parse(&result.text) {
                                    Ok(assistance)=>{
                                        let snapshot=s.current.clone().unwrap_or(result.snapshot);
                                        let entry=generated_lexical_entry(&snapshot,&s.entries,&assistance,&result.provider);
                                        s.entries.push(entry);
                                        s.translation=Some((assistance.translation,result.provider.clone()));
                                        if snapshot.text.split_whitespace().count()>=s.config.typing.min_rephrase_words { s.request(Capability::Rewrite,&events); }
                                    }
                                    Err(error)=>s.notice=error.to_string(),
                                },
                                Capability::Translate=>s.translation=Some((result.text,result.provider)),
                                Capability::Rewrite=>{
                                    let end=result.snapshot.text.chars().count();
                                    s.suggestions.insert(0,Suggestion{snapshot:s.current.clone().unwrap_or(result.snapshot),start:0,end,replacement:result.text,description:"Natural phrasing".into(),kind:SuggestionKind::Rewrite,provider:result.provider});
                                }
                            } s.publish();
                        },
                        Err(e)=>{s.notice=e.to_string();s.publish();}
                    }
                }
            },
            _=heartbeat.tick()=>{s.inspect(&events);s.capture_lease();if let Some(w)=watcher.as_mut()&& w.try_wait().ok().flatten().is_some(){watcher=None;let _=s.ui.try_send(UiEvent::Notice("Automatic selection watching is unavailable; use the selection shortcut".into()));}}
            _=tick.tick()=>{
                if s.search||s.selection||!s.allowed(){continue;}
                let Some(current)=s.current.clone()else{continue;};if current.text.trim().is_empty(){continue;}
                if s.local_revision!=current.revision && s.last_change.elapsed()>=Duration::from_millis(60){
                    s.inspect(&events);let Some(current)=s.current.clone()else{continue;};s.local_revision=current.revision;s.entries=s.dictionary.lookup(writing_word(&current.text)).unwrap_or_default();s.checker.send_replace(Some(current.clone()));
                    let prefix=current.text.split_whitespace().last().unwrap_or("");
                    if prefix.chars().count()>=2&&!current.text.ends_with(char::is_whitespace)
                        && let Ok(words)=s.dictionary.complete(prefix,3){for word in words {if word!=prefix{s.suggestions.push(Suggestion{snapshot:current.clone(),start:current.text.chars().count()-prefix.chars().count(),end:current.text.chars().count(),replacement:word,description:"Word completion".into(),kind:SuggestionKind::Completion,provider:"Local dictionary".into()});}}} s.publish();
                }
                if s.model_revision!=current.revision && s.last_change.elapsed()>=Duration::from_millis(s.config.typing.idle_ms)&&sentence_ready(&current.text){s.request(Capability::WritingAssistance,&events);}
            }
        }
    }
    if let Some(task) = s.model {
        task.abort();
    }
    Ok(())
}
fn start_checker(events: mpsc::Sender<Event>) -> tokio::sync::watch::Sender<Option<TextSnapshot>> {
    let (sender, mut receiver) = tokio::sync::watch::channel::<Option<TextSnapshot>>(None);
    std::thread::Builder::new()
        .name("writy-english".into())
        .spawn(move || {
            let mut checker = EnglishChecker::default();
            let _ = events.blocking_send(Event::CheckerReady);
            loop {
                match receiver.has_changed() {
                    Ok(true) => {
                        let snapshot = receiver.borrow_and_update().clone();
                        if let Some(snapshot) = snapshot {
                            let suggestions = checker.check(&snapshot);
                            if events
                                .blocking_send(Event::Checked(snapshot, suggestions))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    Ok(false) => std::thread::sleep(Duration::from_millis(20)),
                    Err(_) => break,
                }
            }
        })
        .expect("English checking thread");
    sender
}
async fn control_server(
    listener: UnixListener,
    tx: mpsc::Sender<(Command, oneshot::Sender<String>)>,
) {
    while let Ok((stream, _)) = listener.accept().await {
        if !ipc::same_user(&stream) {
            continue;
        }
        let tx = tx.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let result: Result<()> = async {
                let line = tokio::time::timeout(
                    Duration::from_secs(2),
                    ipc::read_line(&mut reader, 65536),
                )
                .await??
                .context("empty command")?;
                let cmd: Command = serde_json::from_str(&line)?;
                let (reply, result) = oneshot::channel();
                tx.send((cmd, reply)).await?;
                let answer = result.await?;
                writer.write_all(format!("{answer}\n").as_bytes()).await?;
                Ok(())
            }
            .await;
            if result.is_err() {
                let _ = writer.write_all(b"invalid command\n").await;
            }
        });
    }
}
async fn bridge_server(listener: UnixListener, tx: mpsc::Sender<Event>) {
    let mut session = 0_u64;
    while let Ok((stream, _)) = listener.accept().await {
        if !ipc::same_user(&stream) {
            continue;
        }
        session = session.wrapping_add(1);
        let session = session;
        let tx = tx.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let (controls, mut control_rx) = mpsc::channel::<serde_json::Value>(8);
            if tx.send(Event::Connected(session, controls)).await.is_err() {
                return;
            }
            // Reading a partially consumed frame must never be cancelled just because a
            // heartbeat is ready. Each half owns its independent, persistent task.
            let read_tx = tx.clone();
            let mut reading = tokio::spawn(async move {
                while let Ok(Some(line)) = ipc::read_line(&mut reader, 1_048_576).await {
                    let Ok(event) = serde_json::from_str(&line) else {
                        break;
                    };
                    if read_tx.send(Event::Bridge(session, event)).await.is_err() {
                        break;
                    }
                }
            });
            let mut writing = tokio::spawn(async move {
                while let Some(control) = control_rx.recv().await {
                    let Ok(mut bytes) = serde_json::to_vec(&control) else {
                        break;
                    };
                    bytes.push(b'\n');
                    if writer.write_all(&bytes).await.is_err() {
                        break;
                    }
                }
            });
            tokio::select! {
                _=&mut reading=>writing.abort(),
                _=&mut writing=>reading.abort(),
            }
            let _ = tx.send(Event::Disconnected(session)).await;
        });
    }
}

async fn cursor_position() -> Option<(i32, i32)> {
    let output = tokio::time::timeout(
        Duration::from_millis(500),
        tokio::process::Command::new("hyprctl")
            .args(["-j", "cursorpos"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let position: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let x = position.get("x")?.as_f64()?;
    let y = position.get("y")?.as_f64()?;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some((x.round() as i32, y.round() as i32))
}

fn sentence_ready(text: &str) -> bool {
    text.trim_end().ends_with('.') && writing_word(text).chars().count() >= 2
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn clicking_search_does_not_clear_results_or_cancel_generation() {
        let (ui, _ui_events) = std::sync::mpsc::sync_channel(32);
        let generation = Arc::new(AtomicU64::new(7));
        let mut state = State {
            config: Config::default(),
            ui,
            generation: generation.clone(),
            buffer: TypingBuffer::default(),
            decoder: Decoder::default(),
            dictionary: Dictionary::open(":memory:").unwrap(),
            checker: tokio::sync::watch::channel(None).0,
            access: Accessibility::start(),
            current: None,
            anchor: None,
            suggestions: vec![],
            entries: vec![],
            translation: None,
            notice: String::new(),
            app: String::new(),
            window: String::new(),
            pid: 0,
            epoch: 0,
            seq: 0,
            prose: false,
            secure: false,
            locked: false,
            bridge: None,
            model: None,
            last_change: Instant::now(),
            local_revision: u64::MAX,
            model_revision: u64::MAX,
            search: false,
            hud_interacting: false,
            selection: false,
            selection_position: None,
            inspecting: false,
            access_status: String::new(),
            checker_ready: false,
            bridge_session: 0,
            output: None,
            output_bounds: None,
        };
        state.search = true;
        let mut buffer = TypingBuffer::default();
        buffer.push("change");
        state.current = Some(buffer.snapshot());
        state.translation = Some(("تغيير".into(), "fixture".into()));
        state.model = Some(tokio::spawn(std::future::pending()));
        let (events, _rx) = mpsc::channel(32);
        state
            .bridge_event(
                BridgeEvent::Reset {
                    seq: 1,
                    focus_epoch: 2,
                    reason: "pointer".into(),
                },
                &events,
            )
            .await;
        assert_eq!(
            state.current.as_ref().map(|s| s.text.as_str()),
            Some("change")
        );
        assert_eq!(
            state.translation.as_ref().map(|t| t.0.as_str()),
            Some("تغيير")
        );
        assert_eq!(generation.load(Ordering::SeqCst), 7);
        assert!(state.model.is_some());
        state
            .bridge_event(
                BridgeEvent::Focus {
                    app_id: "org.example.Other".into(),
                    window_id: "other".into(),
                    focus_epoch: 3,
                    seq: 2,
                    pid: 0,
                    locked: false,
                    monitor: String::new(),
                    monitor_geometry: None,
                },
                &events,
            )
            .await;
        assert!(state.search);
        assert_eq!(
            state.current.as_ref().map(|s| s.text.as_str()),
            Some("change")
        );
        state.clear();
        assert!(state.current.is_none());
        assert!(state.translation.is_none());
    }
    #[tokio::test]
    async fn selecting_hud_text_preserves_panels_and_ignores_self_lookup() {
        let (ui, _ui_events) = std::sync::mpsc::sync_channel(32);
        let generation = Arc::new(AtomicU64::new(7));
        let mut state = State {
            config: Config::default(),
            ui,
            generation: generation.clone(),
            buffer: TypingBuffer::default(),
            decoder: Decoder::default(),
            dictionary: Dictionary::open(":memory:").unwrap(),
            checker: tokio::sync::watch::channel(None).0,
            access: Accessibility::start(),
            current: None,
            anchor: None,
            suggestions: vec![],
            entries: vec![],
            translation: None,
            notice: String::new(),
            app: String::new(),
            window: String::new(),
            pid: 0,
            epoch: 0,
            seq: 0,
            prose: false,
            secure: false,
            locked: false,
            bridge: None,
            model: None,
            last_change: Instant::now(),
            local_revision: u64::MAX,
            model_revision: u64::MAX,
            search: false,
            hud_interacting: false,
            selection: false,
            selection_position: None,
            inspecting: false,
            access_status: String::new(),
            checker_ready: false,
            bridge_session: 0,
            output: None,
            output_bounds: None,
        };
        let (events, _rx) = mpsc::channel(32);
        state
            .command(Command::HudInteraction { active: true }, &events)
            .await;
        state
            .bridge_event(
                BridgeEvent::Reset {
                    seq: 1,
                    focus_epoch: 2,
                    reason: "pointer".into(),
                },
                &events,
            )
            .await;
        assert!(matches!(
            _ui_events.try_iter().last(),
            Some(UiEvent::Clear {
                preserve_huds: true,
                ..
            })
        ));
        let before = generation.load(Ordering::SeqCst);
        assert_eq!(
            state
                .command(
                    Command::Selection {
                        text: Some("selected panel text".into())
                    },
                    &events
                )
                .await,
            "ignored"
        );
        assert_eq!(generation.load(Ordering::SeqCst), before);
        assert!(!state.selection);
    }
    #[test]
    fn automatic_model_waits_for_a_final_dot() {
        for text in [
            "Hello",
            "Hello world",
            "Hello world!",
            "Hello world?",
            "Done. More",
            "",
            ".",
        ] {
            assert!(!sentence_ready(text), "unexpected request for {text:?}");
        }
        for text in [
            "Hello world.",
            "Hello world.  ",
            "مرحبا بالعالم.",
            "First. Second.",
        ] {
            assert!(sentence_ready(text), "missing request for {text:?}");
        }
    }
    use super::*;
    #[test]
    fn generated_details_fill_missing_categories_without_replacing_evidence() {
        let mut buffer = TypingBuffer::default();
        buffer.push("make a decision.");
        let snapshot = buffer.snapshot();
        let assistance=WritingAssistance::parse(r#"{"rewrite":"make a decision.","translation":"اتخذ قرارًا.","definitions":["generated definition"],"examples":["a new example"],"collocations":[],"synonyms":[],"antonyms":[],"word_family":["decide"]}"#).unwrap();
        let entries = vec![LexicalEntry {
            senses: vec![Sense {
                definitions: vec!["dictionary evidence".into()],
                ..Sense::default()
            }],
            ..LexicalEntry::default()
        }];
        let generated = generated_lexical_entry(&snapshot, &entries, &assistance, "fixture model");
        assert_eq!(generated.headword, "decision");
        assert!(generated.senses[0].definitions.is_empty());
        assert_eq!(generated.senses[0].examples, vec!["a new example"]);
        assert!(generated.source.name.starts_with("AI-generated ·"));
        assert_eq!(
            entries[0].senses[0].definitions,
            vec!["dictionary evidence"]
        );
    }

    #[tokio::test]
    async fn heartbeat_does_not_discard_a_partially_received_input_frame() {
        let path =
            std::env::temp_dir().join(format!("writy-frame-test-{}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let _cleanup = ipc::SocketCleanup(path.clone());
        let (tx, mut events) = mpsc::channel(8);
        let server = tokio::spawn(bridge_server(listener, tx));
        let mut client = tokio::net::UnixStream::connect(path).await.unwrap();
        let Event::Connected(session, controls) = events.recv().await.unwrap() else {
            panic!("missing connection");
        };
        client.write_all(b"{\"ty").await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        controls
            .send(serde_json::json!({"type":"capture","enabled":false,"focus_epoch":0}))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        client
            .write_all(b"pe\":\"hello\",\"version\":1,\"seq\":1}\n")
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event,Event::Bridge(id,BridgeEvent::Hello{version:1,..}) if id==session));
        server.abort();
    }
}
