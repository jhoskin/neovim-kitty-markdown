//! Port of `diagrams/sequence/sequenceDb.ts` (mermaid 12.2.0): the
//! `SequenceDB` class, its `LINETYPE` and `PLACEMENT` tables, and `apply`.
//!
//! The parser actions build [`Stmt`] values (the `param` objects that
//! `apply` switches on) and the db applies them in order once the whole
//! document has parsed, as `start -> SD document { yy.apply($2) }` does.

use indexmap::IndexMap;

use crate::mermaid::theme::khroma;

/// `LINETYPE` in `sequenceDb.ts`, with the same numbers so a dump of the
/// db reads like mermaid's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum LineType {
    Solid = 0,
    Dotted = 1,
    Note = 2,
    SolidCross = 3,
    DottedCross = 4,
    SolidOpen = 5,
    DottedOpen = 6,
    LoopStart = 10,
    LoopEnd = 11,
    AltStart = 12,
    AltElse = 13,
    AltEnd = 14,
    OptStart = 15,
    OptEnd = 16,
    ActiveStart = 17,
    ActiveEnd = 18,
    ParStart = 19,
    ParAnd = 20,
    ParEnd = 21,
    RectStart = 22,
    RectEnd = 23,
    SolidPoint = 24,
    DottedPoint = 25,
    Autonumber = 26,
    CriticalStart = 27,
    CriticalOption = 28,
    CriticalEnd = 29,
    BreakStart = 30,
    BreakEnd = 31,
    ParOverStart = 32,
    BidirectionalSolid = 33,
    BidirectionalDotted = 34,
    SolidTop = 41,
    SolidBottom = 42,
    StickTop = 43,
    StickBottom = 44,
    SolidArrowTopReverse = 45,
    SolidArrowBottomReverse = 46,
    StickArrowTopReverse = 47,
    StickArrowBottomReverse = 48,
    SolidTopDotted = 51,
    SolidBottomDotted = 52,
    StickTopDotted = 53,
    StickBottomDotted = 54,
    SolidArrowTopReverseDotted = 55,
    SolidArrowBottomReverseDotted = 56,
    StickArrowTopReverseDotted = 57,
    StickArrowBottomReverseDotted = 58,
    CentralConnection = 59,
    CentralConnectionReverse = 60,
    CentralConnectionDual = 61,
}

impl LineType {
    /// The arrow types that draw a message line: the list the renderer
    /// checks in `buildMessageModel` and before bumping `sequenceIndex`.
    pub fn is_message(self) -> bool {
        use LineType::*;
        matches!(
            self,
            SolidOpen
                | DottedOpen
                | Solid
                | SolidTop
                | SolidBottom
                | StickTop
                | StickBottom
                | SolidTopDotted
                | SolidBottomDotted
                | StickTopDotted
                | StickBottomDotted
                | SolidArrowTopReverse
                | SolidArrowBottomReverse
                | StickArrowTopReverse
                | StickArrowBottomReverse
                | SolidArrowTopReverseDotted
                | SolidArrowBottomReverseDotted
                | StickArrowTopReverseDotted
                | StickArrowBottomReverseDotted
                | Dotted
                | SolidCross
                | DottedCross
                | SolidPoint
                | DottedPoint
                | BidirectionalSolid
                | BidirectionalDotted
        )
    }

    /// The `messageLine1` test in `drawMessage`.
    pub fn is_dotted(self) -> bool {
        use LineType::*;
        matches!(
            self,
            Dotted
                | DottedCross
                | DottedPoint
                | DottedOpen
                | BidirectionalDotted
                | SolidTopDotted
                | SolidBottomDotted
                | StickTopDotted
                | StickBottomDotted
                | SolidArrowTopReverseDotted
                | SolidArrowBottomReverseDotted
                | StickArrowTopReverseDotted
                | StickArrowBottomReverseDotted
        )
    }

    /// `isReverseArrowType` in `sequenceRenderer.ts`.
    pub fn is_reverse(self) -> bool {
        use LineType::*;
        matches!(
            self,
            SolidArrowTopReverse
                | SolidArrowTopReverseDotted
                | SolidArrowBottomReverse
                | SolidArrowBottomReverseDotted
                | StickArrowTopReverse
                | StickArrowTopReverseDotted
                | StickArrowBottomReverse
                | StickArrowBottomReverseDotted
        )
    }

    /// `isBidirectionalArrowType` in `sequenceRenderer.ts`.
    pub fn is_bidirectional(self) -> bool {
        matches!(
            self,
            LineType::BidirectionalSolid | LineType::BidirectionalDotted
        )
    }

    /// The types `buildMessageModel` leaves at full length because they
    /// have no arrowhead at the end, or an open one.
    pub fn keeps_end(self) -> bool {
        use LineType::*;
        matches!(
            self,
            SolidOpen
                | DottedOpen
                | StickTop
                | StickBottom
                | StickTopDotted
                | StickBottomDotted
                | SolidArrowTopReverseDotted
                | SolidArrowBottomReverseDotted
                | StickArrowTopReverse
                | StickArrowBottomReverse
                | StickArrowTopReverseDotted
                | StickArrowBottomReverseDotted
                | SolidArrowTopReverse
                | SolidArrowBottomReverse
        )
    }

    /// The types `buildMessageModel` shortens at the start for a second
    /// arrowhead.
    pub fn shortens_start(self) -> bool {
        use LineType::*;
        matches!(
            self,
            BidirectionalSolid
                | BidirectionalDotted
                | SolidArrowTopReverseDotted
                | SolidArrowBottomReverseDotted
                | SolidArrowTopReverse
                | SolidArrowBottomReverse
        )
    }

    pub fn is_central_connection(self) -> bool {
        matches!(
            self,
            LineType::CentralConnection
                | LineType::CentralConnectionReverse
                | LineType::CentralConnectionDual
        )
    }
}

/// `PLACEMENT` in `sequenceDb.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    LeftOf = 0,
    RightOf = 1,
    Over = 2,
}

/// The `{ text, wrap }` object `parseMessage` returns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedMessage {
    pub text: String,
    pub wrap: Option<bool>,
}

/// `parseBoxData`'s result.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxData {
    pub text: Option<String>,
    pub color: String,
    pub wrap: Option<bool>,
}

/// A participant declaration as the grammar builds it: the
/// `{ type: 'addParticipant', actor, draw, description, config }` object.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActorRef {
    pub actor: String,
    pub draw: Option<String>,
    pub description: Option<ParsedMessage>,
    pub config: Option<String>,
}

/// One `param` of `SequenceDB.apply`, with the source line it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub line: usize,
    pub kind: StmtKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    SequenceIndex {
        start: Option<f64>,
        step: Option<f64>,
        visible: bool,
    },
    AddParticipant(ActorRef),
    CreateParticipant(ActorRef),
    DestroyParticipant(ActorRef),
    ActiveStart(String),
    ActiveEnd(String),
    CentralConnection(String),
    CentralConnectionReverse(String),
    AddNote {
        actors: Vec<String>,
        placement: Placement,
        text: ParsedMessage,
    },
    AddLinks {
        actor: String,
        text: ParsedMessage,
    },
    AddALink {
        actor: String,
        text: ParsedMessage,
    },
    AddProperties {
        actor: String,
        text: ParsedMessage,
    },
    AddDetails {
        actor: String,
        text: ParsedMessage,
    },
    AddMessage {
        from: String,
        to: String,
        signal_type: LineType,
        msg: ParsedMessage,
        activate: bool,
        central_connection: Option<LineType>,
    },
    BoxStart(BoxData),
    BoxEnd,
    /// `loopStart`, `optStart`, `altStart`, `else`, `parStart`, `and`,
    /// `criticalStart`, `option`, `breakStart`, and `rectStart`: a block
    /// marker with a label (the rect's color travels as its label).
    BlockStart {
        signal_type: LineType,
        text: ParsedMessage,
    },
    /// `loopEnd`, `rectEnd`, `optEnd`, `altEnd`, `parEnd`, `criticalEnd`,
    /// and `breakEnd`.
    BlockEnd(LineType),
    Title(String),
    AccTitle(String),
    AccDescription(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Actor {
    pub name: String,
    pub description: String,
    pub wrap: bool,
    pub prev_actor: Option<String>,
    pub next_actor: Option<String>,
    /// Index into [`SequenceDb::boxes`].
    pub box_index: Option<usize>,
    /// `PARTICIPANT_TYPE`: `participant`, `actor`, `boundary`, `control`,
    /// `database`, `entity`, `collections`, or `queue`.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceBox {
    pub name: Option<String>,
    pub wrap: bool,
    pub fill: String,
    pub actor_keys: Vec<String>,
}

/// The `message` field of an `AUTONUMBER` entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Autonumber {
    pub start: Option<f64>,
    pub step: Option<f64>,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: usize,
    pub from: Option<String>,
    pub to: Option<String>,
    pub message: String,
    pub autonumber: Option<Autonumber>,
    pub wrap: bool,
    pub kind: Option<LineType>,
    pub activate: bool,
    pub placement: Option<Placement>,
    pub central_connection: Option<LineType>,
}

impl Message {
    fn blank(id: usize) -> Message {
        Message {
            id,
            from: None,
            to: None,
            message: String::new(),
            autonumber: None,
            wrap: false,
            kind: None,
            activate: false,
            placement: None,
            central_connection: None,
        }
    }
}

/// An error the db throws; mermaid surfaces `Error.message` unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbError {
    pub message: String,
}

impl DbError {
    fn new(message: impl Into<String>) -> DbError {
        DbError {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SequenceDb {
    pub actors: IndexMap<String, Actor>,
    /// Actor name to the message index at which it is created.
    pub created_actors: IndexMap<String, usize>,
    pub destroyed_actors: IndexMap<String, usize>,
    pub boxes: Vec<SequenceBox>,
    pub messages: Vec<Message>,
    pub sequence_numbers_enabled: bool,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_description: Option<String>,
    /// `setWrap`'s value; `None` until a `%%{wrap}%%` directive sets it.
    wrap_enabled: Option<bool>,
    /// `getConfig().sequence.wrap`, the fallback of `autoWrap`.
    config_wrap: bool,
    prev_actor: Option<String>,
    current_box: Option<usize>,
    last_created: Option<String>,
    last_destroyed: Option<String>,
}

impl SequenceDb {
    /// `new SequenceDB()` after `setWrap(getConfig().wrap)`, with the
    /// `sequence.wrap` fallback of `autoWrap` captured.
    pub fn new(config_wrap: bool, wrap_directive: Option<bool>) -> SequenceDb {
        SequenceDb {
            wrap_enabled: wrap_directive,
            config_wrap,
            ..SequenceDb::default()
        }
    }

    /// `autoWrap`.
    pub fn auto_wrap(&self) -> bool {
        self.wrap_enabled.unwrap_or(self.config_wrap)
    }

    pub fn has_at_least_one_box(&self) -> bool {
        !self.boxes.is_empty()
    }

    pub fn has_at_least_one_box_with_title(&self) -> bool {
        self.boxes
            .iter()
            .any(|b| b.name.as_deref().is_some_and(|n| !n.is_empty()))
    }

    pub fn actor_keys(&self) -> Vec<String> {
        self.actors.keys().cloned().collect()
    }

    /// `extractWrap`: a `wrap:` or `nowrap:` prefix, with an optional
    /// leading colon, decides the wrap flag and leaves the text.
    fn extract_wrap(text: &str) -> (String, Option<bool>) {
        let text = text.trim();
        fn strip(t: &str) -> &str {
            t.strip_prefix(':').unwrap_or(t)
        }
        let (wrap, rest) = if let Some(rest) = strip(text).strip_prefix("wrap:") {
            (Some(true), rest)
        } else if let Some(rest) = strip(text).strip_prefix("nowrap:") {
            (Some(false), rest)
        } else {
            (None, text)
        };
        (rest.trim().to_string(), wrap)
    }

    /// `parseMessage`.
    pub fn parse_message(text: &str) -> ParsedMessage {
        let (text, wrap) = SequenceDb::extract_wrap(text.trim());
        ParsedMessage { text, wrap }
    }

    /// `parseBoxData`: the first word (or an `rgb()`/`hsl()` call) is the
    /// color when CSS accepts it, the rest is the title; otherwise the
    /// whole line is the title and the box is transparent.
    pub fn parse_box_data(text: &str) -> BoxData {
        let (color, title) = split_box_color(text);
        let (color, title) = if css_color_supported(&color) {
            (color, title)
        } else {
            ("transparent".to_string(), Some(text.trim().to_string()))
        };
        let (text, wrap) = match title {
            Some(t) => {
                let (cleaned, wrap) = SequenceDb::extract_wrap(&t);
                ((!cleaned.is_empty()).then(|| sanitize_text(&cleaned)), wrap)
            }
            None => (None, None),
        };
        BoxData { text, color, wrap }
    }

    fn add_box(&mut self, data: &BoxData) {
        self.boxes.push(SequenceBox {
            name: data.text.clone(),
            wrap: data.wrap.unwrap_or_else(|| self.auto_wrap()),
            fill: data.color.clone(),
            actor_keys: Vec::new(),
        });
        self.current_box = Some(self.boxes.len() - 1);
    }

    /// `addActor`. `metadata` is the `@{ ... }` config, parsed as YAML
    /// with the JSON schema; only `type` and `alias` matter.
    fn add_actor(&mut self, r: &ActorRef) -> Result<(), DbError> {
        let id = r.actor.as_str();
        let name = id;
        let mut description = r.description.clone();
        let mut kind = r.draw.clone();
        if let Some(metadata) = &r.config {
            let yaml = if metadata.contains('\n') {
                format!("{metadata}\n")
            } else {
                format!("{{\n{metadata}\n}}")
            };
            let doc: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(&yaml).map_err(|e| DbError::new(format!("{e}")))?;
            if let Some(t) = doc.get("type").and_then(|v| v.as_str()) {
                kind = Some(t.to_string());
            }
            if let Some(alias) = doc.get("alias").and_then(|v| v.as_str())
                && description.as_ref().is_none_or(|d| d.text == name)
            {
                description = Some(ParsedMessage {
                    text: alias.to_string(),
                    wrap: description.as_ref().and_then(|d| d.wrap),
                });
            }
        }

        let mut assigned_box = self.current_box;
        if let Some(old) = self.actors.get(id) {
            if let (Some(current), Some(old_box)) = (self.current_box, old.box_index)
                && current != old_box
            {
                return Err(DbError::new(format!(
                    "A same participant should only be defined in one Box: {} can't be in '{}' and in '{}' at the same time.",
                    old.name,
                    self.boxes[old_box].name.clone().unwrap_or_default(),
                    self.boxes[current].name.clone().unwrap_or_default()
                )));
            }
            assigned_box = old.box_index.or(self.current_box);
            let old_name = old.name.clone();
            if let Some(actor) = self.actors.get_mut(id) {
                actor.box_index = assigned_box;
            }
            // Don't allow description nulling.
            if name == old_name && description.is_none() {
                return Ok(());
            }
        }

        let description = match description {
            Some(d) if kind.is_some() => d,
            _ => ParsedMessage {
                text: name.to_string(),
                wrap: None,
            },
        };
        let actor = Actor {
            name: name.to_string(),
            description: description.text,
            wrap: description.wrap.unwrap_or_else(|| self.auto_wrap()),
            prev_actor: self.prev_actor.clone(),
            next_actor: None,
            box_index: assigned_box,
            kind: kind.unwrap_or_else(|| "participant".to_string()),
        };
        self.actors.insert(id.to_string(), actor);
        if let Some(prev) = self.prev_actor.clone()
            && let Some(prev_actor) = self.actors.get_mut(&prev)
        {
            prev_actor.next_actor = Some(id.to_string());
        }
        if let Some(b) = self.current_box {
            self.boxes[b].actor_keys.push(id.to_string());
        }
        self.prev_actor = Some(id.to_string());
        Ok(())
    }

    /// `activationCount`: open activations of `part` so far.
    fn activation_count(&self, part: &str) -> i64 {
        if part.is_empty() {
            return 0;
        }
        let mut count = 0;
        for m in &self.messages {
            if m.from.as_deref() == Some(part) {
                match m.kind {
                    Some(LineType::ActiveStart) => count += 1,
                    Some(LineType::ActiveEnd) => count -= 1,
                    _ => {}
                }
            }
        }
        count
    }

    /// `addSignal`.
    fn add_signal(
        &mut self,
        from: Option<&str>,
        to: Option<&str>,
        message: Option<&ParsedMessage>,
        kind: LineType,
        activate: bool,
        central_connection: Option<LineType>,
    ) -> Result<(), DbError> {
        if kind == LineType::ActiveEnd {
            let cnt = self.activation_count(from.unwrap_or(""));
            if cnt < 1 {
                return Err(DbError::new(format!(
                    "Trying to inactivate an inactive participant ({})",
                    from.unwrap_or("undefined")
                )));
            }
        }
        let id = self.messages.len();
        self.messages.push(Message {
            from: from.map(str::to_string),
            to: to.map(str::to_string),
            message: message.map(|m| m.text.clone()).unwrap_or_default(),
            wrap: message
                .and_then(|m| m.wrap)
                .unwrap_or_else(|| self.auto_wrap()),
            kind: Some(kind),
            activate,
            central_connection,
            ..Message::blank(id)
        });
        Ok(())
    }

    /// `addNote`: `actor` is one name or the pair of an `over A,B`.
    fn add_note(&mut self, actors: &[String], placement: Placement, message: &ParsedMessage) {
        let from = actors.first().cloned();
        let to = actors.get(1).cloned().or_else(|| from.clone());
        let id = self.messages.len();
        self.messages.push(Message {
            from,
            to,
            message: message.text.clone(),
            wrap: message.wrap.unwrap_or_else(|| self.auto_wrap()),
            kind: Some(LineType::Note),
            placement: Some(placement),
            ..Message::blank(id)
        });
    }

    /// `apply` for a list of statements, in order.
    pub fn apply_all(&mut self, stmts: &[Stmt]) -> Result<(), (usize, DbError)> {
        for stmt in stmts {
            self.apply(&stmt.kind).map_err(|e| (stmt.line, e))?;
        }
        Ok(())
    }

    /// One case of the `switch (param.type)` in `apply`.
    pub fn apply(&mut self, stmt: &StmtKind) -> Result<(), DbError> {
        match stmt {
            StmtKind::SequenceIndex {
                start,
                step,
                visible,
            } => {
                let id = self.messages.len();
                self.messages.push(Message {
                    autonumber: Some(Autonumber {
                        start: *start,
                        step: *step,
                        visible: *visible,
                    }),
                    kind: Some(LineType::Autonumber),
                    ..Message::blank(id)
                });
            }
            StmtKind::AddParticipant(actor) => self.add_actor(actor)?,
            StmtKind::CreateParticipant(actor) => {
                if self.actors.contains_key(&actor.actor) {
                    return Err(DbError::new(
                        "It is not possible to have actors with the same id, even if one is destroyed before the next is created. Use 'AS' aliases to simulate the behavior",
                    ));
                }
                self.last_created = Some(actor.actor.clone());
                self.add_actor(actor)?;
                self.created_actors
                    .insert(actor.actor.clone(), self.messages.len());
            }
            StmtKind::DestroyParticipant(actor) => {
                self.last_destroyed = Some(actor.actor.clone());
                self.destroyed_actors
                    .insert(actor.actor.clone(), self.messages.len());
            }
            StmtKind::ActiveStart(actor) => {
                self.add_signal(Some(actor), None, None, LineType::ActiveStart, false, None)?
            }
            StmtKind::CentralConnection(actor) => self.add_signal(
                Some(actor),
                None,
                None,
                LineType::CentralConnection,
                false,
                None,
            )?,
            StmtKind::CentralConnectionReverse(actor) => self.add_signal(
                Some(actor),
                None,
                None,
                LineType::CentralConnectionReverse,
                false,
                None,
            )?,
            StmtKind::ActiveEnd(actor) => {
                self.add_signal(Some(actor), None, None, LineType::ActiveEnd, false, None)?
            }
            StmtKind::AddNote {
                actors,
                placement,
                text,
            } => self.add_note(actors, *placement, text),
            // Links, properties, and details feed the popup menus and
            // the `class`/`icon` properties, which the static render has
            // no use for; the oracle hides the menus too.
            StmtKind::AddLinks { .. }
            | StmtKind::AddALink { .. }
            | StmtKind::AddProperties { .. }
            | StmtKind::AddDetails { .. } => {}
            StmtKind::AddMessage {
                from,
                to,
                signal_type,
                msg,
                activate,
                central_connection,
            } => {
                if let Some(created) = self.last_created.clone() {
                    if to != &created {
                        return Err(DbError::new(format!(
                            "The created participant {} does not have an associated creating message after its declaration. Please check the sequence diagram.",
                            self.actors
                                .get(&created)
                                .map_or("undefined", |a| a.name.as_str())
                        )));
                    }
                    self.last_created = None;
                } else if let Some(destroyed) = self.last_destroyed.clone() {
                    if to != &destroyed && from != &destroyed {
                        return Err(DbError::new(format!(
                            "The destroyed participant {} does not have an associated destroying message after its declaration. Please check the sequence diagram.",
                            self.actors
                                .get(&destroyed)
                                .map_or("undefined", |a| a.name.as_str())
                        )));
                    }
                    self.last_destroyed = None;
                }
                self.add_signal(
                    Some(from),
                    Some(to),
                    Some(msg),
                    *signal_type,
                    *activate,
                    *central_connection,
                )?;
            }
            StmtKind::BoxStart(data) => self.add_box(data),
            StmtKind::BoxEnd => self.current_box = None,
            StmtKind::BlockStart { signal_type, text } => {
                self.add_signal(None, None, Some(text), *signal_type, false, None)?
            }
            StmtKind::BlockEnd(kind) => self.add_signal(None, None, None, *kind, false, None)?,
            StmtKind::Title(title) => self.title = Some(sanitize_text(title)),
            StmtKind::AccTitle(t) => self.acc_title = Some(sanitize_text(t)),
            StmtKind::AccDescription(t) => self.acc_description = Some(sanitize_text(t)),
        }
        Ok(())
    }
}

/// The regex of `parseBoxData`:
/// `^((?:rgba?|hsla?)\s*\(.*\)|\w*)(.*)$`.
fn split_box_color(text: &str) -> (String, Option<String>) {
    let lower = text.to_ascii_lowercase();
    let func = ["rgba", "rgb", "hsla", "hsl"]
        .iter()
        .find(|f| lower.starts_with(*f))
        .and_then(|f| {
            let after = &text[f.len()..];
            let open = after.find('(')?;
            if !after[..open].trim().is_empty() {
                return None;
            }
            // `.*` is greedy: the color runs to the last `)` on the line.
            let close = text.rfind(')')?;
            Some(close + 1)
        });
    let end = func.unwrap_or_else(|| {
        text.char_indices()
            .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
            .map_or(text.len(), |(i, _)| i)
    });
    let color = text[..end].trim().to_string();
    let rest = text[end..].trim();
    let title = (!rest.is_empty()).then(|| rest.to_string());
    (color, title)
}

/// `CSS.supports('color', value)` as far as khroma can tell: hex, `rgb`,
/// `hsl`, the named colors, and `transparent`.
fn css_color_supported(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("currentcolor") {
        return true;
    }
    khroma::parse(value).is_ok()
}

/// `sanitizeText` under `securityLevel: strict`: DOMPurify serializes the
/// text back as HTML, so the markup characters come out as entities.
pub fn sanitize_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_message_extracts_wrap_prefixes() {
        assert_eq!(
            SequenceDb::parse_message(" wrap: hello "),
            ParsedMessage {
                text: "hello".into(),
                wrap: Some(true)
            }
        );
        assert_eq!(
            SequenceDb::parse_message(":nowrap:x"),
            ParsedMessage {
                text: "x".into(),
                wrap: Some(false)
            }
        );
        assert_eq!(SequenceDb::parse_message("plain").wrap, None);
    }

    #[test]
    fn box_data_splits_color_and_title() {
        let b = SequenceDb::parse_box_data("Purple Alice & John");
        assert_eq!(b.color, "Purple");
        assert_eq!(b.text.as_deref(), Some("Alice &amp; John"));
        let b = SequenceDb::parse_box_data("Group 1");
        assert_eq!(b.color, "transparent");
        assert_eq!(b.text.as_deref(), Some("Group 1"));
        let b = SequenceDb::parse_box_data("rgb(33,66,99) Title");
        assert_eq!(b.color, "rgb(33,66,99)");
        assert_eq!(b.text.as_deref(), Some("Title"));
        let b = SequenceDb::parse_box_data("transparent Aqua");
        assert_eq!(b.color, "transparent");
        assert_eq!(b.text.as_deref(), Some("Aqua"));
    }

    #[test]
    fn inactive_participant_is_an_error() {
        let mut db = SequenceDb::new(false, None);
        let err = db
            .apply(&StmtKind::ActiveEnd("A".into()))
            .expect_err("unbalanced");
        assert_eq!(
            err.message,
            "Trying to inactivate an inactive participant (A)"
        );
    }

    #[test]
    fn implicit_actors_keep_their_description() {
        let mut db = SequenceDb::new(false, None);
        db.apply(&StmtKind::AddParticipant(ActorRef {
            actor: "A".into(),
            draw: Some("participant".into()),
            description: Some(SequenceDb::parse_message("Alice")),
            config: None,
        }))
        .expect("declare");
        db.apply(&StmtKind::AddParticipant(ActorRef {
            actor: "A".into(),
            ..ActorRef::default()
        }))
        .expect("mention");
        assert_eq!(db.actors["A"].description, "Alice");
        assert_eq!(db.actors.len(), 1);
    }
}
