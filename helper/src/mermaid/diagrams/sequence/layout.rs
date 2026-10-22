//! Port of `diagrams/sequence/sequenceRenderer.ts` (mermaid 12.2.0): the
//! `bounds` object, the model builders, and `draw`'s message loop.
//!
//! mermaid lays out and draws in one pass over the messages, reading
//! heights back from the DOM as it goes; [`Renderer`] keeps that order and
//! takes the heights from the measurer instead. Elements are collected
//! into two lists: `front` holds what d3 `.lower()`ed, in lowering order,
//! and `main` holds everything appended in document order; the scene root
//! is `front` reversed followed by `main`.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::mermaid::scene::{
    Circle, Common, Element, Group, Line, Path, Text, TextAnchor, TextRun,
};
use crate::mermaid::styles::sequence::SequenceStyles;
use crate::text::TextMetrics;
use crate::text::markdown::split_breaks;
use crate::text::wrap::wrap_label;

use super::conf::{Conf, FontPair};
use super::db::{LineType, Message, Placement, SequenceDb};
use super::draw::{self, ACTOR_TYPE_WIDTH, DrawCtx, TextData, js_round};

/// `common.getMax`: `Math.max` over the defined values.
fn get_max(values: &[Option<f64>]) -> Option<f64> {
    values.iter().flatten().copied().reduce(f64::max)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorModel {
    pub key: String,
    pub name: String,
    pub description: String,
    pub wrap: bool,
    pub kind: String,
    pub prev_actor: Option<String>,
    pub next_actor: Option<String>,
    pub box_index: Option<usize>,
    pub width: f64,
    pub height: f64,
    pub margin: Option<f64>,
    pub x: f64,
    pub starty: f64,
    pub stopy: Option<f64>,
    pub actor_cnt: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxModel {
    pub name: Option<String>,
    pub wrap: bool,
    pub fill: String,
    pub actor_keys: Vec<String>,
    pub margin: f64,
    pub text_max_height: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub startx: f64,
    pub starty: f64,
    pub stopx: f64,
    pub stopy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub y: f64,
    pub height: f64,
}

/// `bounds.createLoop`'s object, also the model of a `rect` block.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopModel {
    pub startx: Option<f64>,
    pub starty: f64,
    pub stopx: Option<f64>,
    pub stopy: Option<f64>,
    pub title: Option<String>,
    pub wrap: bool,
    pub width: Option<f64>,
    pub height: f64,
    pub fill: Option<String>,
    pub sections: Vec<Section>,
    pub section_titles: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Activation {
    pub startx: f64,
    pub starty: f64,
    pub stopx: f64,
    pub stopy: Option<f64>,
    pub actor: String,
    /// Index into `Renderer::main` of the anchor group.
    pub anchored: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MsgModel {
    pub width: f64,
    pub height: f64,
    pub startx: f64,
    pub stopx: f64,
    pub starty: f64,
    pub stopy: f64,
    pub message: String,
    pub kind: LineType,
    pub wrap: bool,
    pub from_bounds: f64,
    pub to_bounds: f64,
    pub id: usize,
    pub from: String,
    pub to: String,
    pub sequence_index: f64,
    pub sequence_visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteModel {
    pub width: f64,
    pub height: f64,
    pub startx: f64,
    pub stopx: f64,
    pub starty: f64,
    pub stopy: f64,
    pub message: String,
}

/// One entry of `calculateLoopBounds`' result.
#[derive(Debug, Clone, PartialEq)]
struct LoopWidth {
    from: f64,
    to: f64,
    width: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoundsData {
    pub startx: Option<f64>,
    pub stopx: Option<f64>,
    pub starty: Option<f64>,
    pub stopy: Option<f64>,
}

/// `bounds` in `sequenceRenderer.ts`.
#[derive(Debug, Clone, Default)]
pub struct Bounds {
    pub data: BoundsData,
    pub vertical_pos: f64,
    pub sequence_items: Vec<LoopModel>,
    pub activations: Vec<Activation>,
}

fn update_val(slot: &mut Option<f64>, val: f64, fun: fn(f64, f64) -> f64) {
    *slot = Some(match *slot {
        None => val,
        Some(cur) => fun(val, cur),
    });
}

impl Bounds {
    /// `updateBounds`: grow every open loop and activation by the nesting
    /// depth times `boxMargin`.
    fn update_bounds(&mut self, conf: &Conf, startx: f64, starty: f64, stopx: f64, stopy: f64) {
        let items = self.sequence_items.len();
        let mut cnt = 0usize;
        for item in &mut self.sequence_items {
            cnt += 1;
            let n = (items - cnt + 1) as f64;
            update_val(&mut item.startx, startx - n * conf.box_margin, f64::min);
            update_val(&mut item.stopx, stopx + n * conf.box_margin, f64::max);
            item.starty = item.starty.min(starty - n * conf.box_margin);
            update_val(&mut item.stopy, stopy + n * conf.box_margin, f64::max);
            update_val(
                &mut self.data.startx,
                startx - n * conf.box_margin,
                f64::min,
            );
            update_val(&mut self.data.stopx, stopx + n * conf.box_margin, f64::max);
            update_val(
                &mut self.data.starty,
                starty - n * conf.box_margin,
                f64::min,
            );
            update_val(&mut self.data.stopy, stopy + n * conf.box_margin, f64::max);
        }
        for activation in &mut self.activations {
            cnt += 1;
            let n = items as f64 - cnt as f64 + 1.0;
            activation.starty = activation.starty.min(starty - n * conf.box_margin);
            update_val(&mut activation.stopy, stopy + n * conf.box_margin, f64::max);
            update_val(
                &mut self.data.startx,
                startx - n * conf.box_margin,
                f64::min,
            );
            update_val(&mut self.data.stopx, stopx + n * conf.box_margin, f64::max);
        }
    }

    pub fn insert(&mut self, conf: &Conf, startx: f64, starty: f64, stopx: f64, stopy: f64) {
        let (sx, ex) = (startx.min(stopx), startx.max(stopx));
        let (sy, ey) = (starty.min(stopy), starty.max(stopy));
        update_val(&mut self.data.startx, sx, f64::min);
        update_val(&mut self.data.starty, sy, f64::min);
        update_val(&mut self.data.stopx, ex, f64::max);
        update_val(&mut self.data.stopy, ey, f64::max);
        self.update_bounds(conf, sx, sy, ex, ey);
    }

    pub fn bump_vertical_pos(&mut self, bump: f64) {
        self.vertical_pos += bump;
        self.data.stopy = get_max(&[self.data.stopy, Some(self.vertical_pos)]);
    }

    fn actor_activations(&self, actor: &str) -> usize {
        self.activations.iter().filter(|a| a.actor == actor).count()
    }

    fn create_loop(&self, title: Option<&Message>, fill: Option<String>) -> LoopModel {
        LoopModel {
            startx: None,
            starty: self.vertical_pos,
            stopx: None,
            stopy: None,
            title: title.map(|m| m.message.clone()),
            wrap: title.is_some_and(|m| m.wrap),
            width: None,
            height: 0.0,
            fill,
            sections: Vec::new(),
            section_titles: Vec::new(),
        }
    }

    fn new_loop(&mut self, title: Option<&Message>, fill: Option<String>) {
        let model = self.create_loop(title, fill);
        self.sequence_items.push(model);
    }

    fn end_loop(&mut self) -> Option<LoopModel> {
        self.sequence_items.pop()
    }

    fn add_section_to_loop(&mut self, message: &Message) {
        let y = self.vertical_pos;
        if let Some(l) = self.sequence_items.last_mut() {
            l.sections.push(Section { y, height: 0.0 });
            l.section_titles.push(Some(message.message.clone()));
        }
    }
}

/// `calculateTextDimensions` bound to a font role.
fn text_dims(metrics: &TextMetrics<'_>, text: &str, font: &FontPair) -> (f64, f64) {
    let d = metrics.calculate_text_dimensions(text, &font.measure);
    (f64::from(d.width), f64::from(d.height))
}

/// `utils.wrapLabel` with `calculateTextWidth` in the role's font.
fn wrap(metrics: &TextMetrics<'_>, label: &str, max_width: f64, font: &FontPair) -> String {
    wrap_label(label, max_width as f32, "<br/>", &|t| {
        metrics.calculate_text_width(t, &font.measure)
    })
}

pub struct Renderer<'a> {
    pub conf: Conf,
    pub styles: &'a SequenceStyles,
    pub metrics: &'a TextMetrics<'a>,
    pub db: &'a SequenceDb,
    pub actors: IndexMap<String, ActorModel>,
    pub actor_keys: Vec<String>,
    pub boxes: Vec<BoxModel>,
    pub messages: Vec<Message>,
    pub bounds: Bounds,
    /// Elements d3 `.lower()`ed, in lowering order.
    pub front: Vec<Element>,
    pub main: Vec<Element>,
    /// `bounds.models.boxes`: indices into `boxes`, in the order
    /// `addActorRenderingData` closed them.
    model_boxes: Vec<usize>,
    msg_models: HashMap<usize, MsgModel>,
    note_models: HashMap<usize, NoteModel>,
    loop_widths: HashMap<usize, LoopWidth>,
    /// `msg.width` set by `adjustLoopHeightForWrap`.
    msg_widths: HashMap<usize, f64>,
    actor_cnt: usize,
    /// Indices into `front` of the top actors' lifeline groups, keyed by
    /// actor, for `fixLifeLineHeights`.
    lifelines: HashMap<String, usize>,
    /// The `backgrounds` list of `draw`: closed `rect` blocks, drawn after
    /// the actors.
    backgrounds: Vec<LoopModel>,
}

impl<'a> Renderer<'a> {
    pub fn new(
        conf: Conf,
        styles: &'a SequenceStyles,
        metrics: &'a TextMetrics<'a>,
        db: &'a SequenceDb,
    ) -> Renderer<'a> {
        let actors = db
            .actors
            .iter()
            .map(|(key, a)| {
                (
                    key.clone(),
                    ActorModel {
                        key: key.clone(),
                        name: a.name.clone(),
                        description: a.description.clone(),
                        wrap: a.wrap,
                        kind: a.kind.clone(),
                        prev_actor: a.prev_actor.clone(),
                        next_actor: a.next_actor.clone(),
                        box_index: a.box_index,
                        width: 0.0,
                        height: 0.0,
                        margin: None,
                        x: 0.0,
                        starty: 0.0,
                        stopy: None,
                        actor_cnt: None,
                    },
                )
            })
            .collect();
        let boxes = db
            .boxes
            .iter()
            .map(|b| BoxModel {
                name: b.name.clone(),
                wrap: b.wrap,
                fill: b.fill.clone(),
                actor_keys: b.actor_keys.clone(),
                margin: 0.0,
                text_max_height: 0.0,
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                startx: 0.0,
                starty: 0.0,
                stopx: 0.0,
                stopy: 0.0,
            })
            .collect();
        Renderer {
            conf,
            styles,
            metrics,
            db,
            actors,
            actor_keys: db.actor_keys(),
            boxes,
            messages: db.messages.clone(),
            bounds: Bounds::default(),
            front: Vec::new(),
            main: Vec::new(),
            model_boxes: Vec::new(),
            msg_models: HashMap::new(),
            note_models: HashMap::new(),
            loop_widths: HashMap::new(),
            msg_widths: HashMap::new(),
            actor_cnt: 0,
            lifelines: HashMap::new(),
            backgrounds: Vec::new(),
        }
    }

    fn ctx(&self) -> DrawCtx<'_> {
        DrawCtx {
            conf: &self.conf,
            styles: self.styles,
            metrics: self.metrics,
        }
    }

    fn actor(&self, key: &str) -> Option<&ActorModel> {
        self.actors.get(key)
    }

    fn actor_center(&self, key: &str) -> f64 {
        self.actor(key).map_or(0.0, |a| a.x + a.width / 2.0)
    }

    /// `getMaxMessageWidthPerActor`.
    fn max_message_width_per_actor(&self) -> IndexMap<String, f64> {
        let conf = &self.conf;
        let mut out: IndexMap<String, f64> = IndexMap::new();
        let mut bump = |key: Option<&str>, width: f64| {
            let key = key.unwrap_or("undefined").to_string();
            let cur = out.get(&key).copied().unwrap_or(0.0);
            out.insert(key, cur.max(width));
        };
        for msg in &self.messages {
            let (Some(to), Some(from)) = (msg.to.as_deref(), msg.from.as_deref()) else {
                continue;
            };
            let (Some(actor), Some(_)) = (self.actor(to), self.actor(from)) else {
                continue;
            };
            if msg.placement == Some(Placement::LeftOf) && actor.prev_actor.is_none() {
                continue;
            }
            if msg.placement == Some(Placement::RightOf) && actor.next_actor.is_none() {
                continue;
            }
            let is_note = msg.placement.is_some();
            let font = if is_note {
                &conf.fonts.note
            } else {
                &conf.fonts.message
            };
            let wrapped = if msg.wrap {
                wrap(
                    self.metrics,
                    &msg.message,
                    conf.width - 2.0 * conf.wrap_padding,
                    font,
                )
            } else {
                msg.message.clone()
            };
            let (w, _) = text_dims(self.metrics, &wrapped, font);
            let message_width = w + 2.0 * conf.wrap_padding;
            let next = actor.next_actor.as_deref();
            let prev = actor.prev_actor.as_deref();
            if !is_note && Some(from) == next {
                bump(Some(to), message_width);
            } else if !is_note && Some(from) == prev {
                bump(Some(from), message_width);
            } else if !is_note && from == to {
                bump(Some(from), message_width / 2.0);
                bump(Some(to), message_width / 2.0);
            } else if msg.placement == Some(Placement::RightOf) {
                bump(Some(from), message_width);
            } else if msg.placement == Some(Placement::LeftOf) {
                bump(prev, message_width);
            } else if msg.placement == Some(Placement::Over) {
                if prev.is_some() {
                    bump(prev, message_width / 2.0);
                }
                if next.is_some() {
                    bump(Some(from), message_width / 2.0);
                }
            }
        }
        out
    }

    /// `calculateActorMargins`: returns the new `conf.height`.
    fn calculate_actor_margins(&mut self, widths: &IndexMap<String, f64>) -> f64 {
        let conf = self.conf.clone();
        let mut max_height: f64 = 0.0;
        for actor in self.actors.values_mut() {
            if actor.wrap {
                actor.description = wrap(
                    self.metrics,
                    &actor.description,
                    conf.width - 2.0 * conf.wrap_padding,
                    &conf.fonts.actor,
                );
            }
            let (w, h) = text_dims(self.metrics, &actor.description, &conf.fonts.actor);
            actor.width = if actor.wrap {
                conf.width
            } else {
                conf.width.max(w + 2.0 * conf.wrap_padding)
            };
            actor.height = if actor.wrap {
                h.max(conf.height)
            } else {
                conf.height
            };
            max_height = max_height.max(actor.height);
        }

        for (key, message_width) in widths {
            let Some(actor) = self.actors.get(key) else {
                continue;
            };
            let next = actor.next_actor.as_deref().and_then(|n| self.actors.get(n));
            let margin = match next {
                None => {
                    (message_width + conf.actor_margin - actor.width / 2.0).max(conf.actor_margin)
                }
                Some(next) => {
                    (message_width + conf.actor_margin - actor.width / 2.0 - next.width / 2.0)
                        .max(conf.actor_margin)
                }
            };
            if let Some(actor) = self.actors.get_mut(key) {
                actor.margin = Some(margin);
            }
        }

        let mut max_box_height: f64 = 0.0;
        for b in &mut self.boxes {
            let mut total_width: f64 = b
                .actor_keys
                .iter()
                .filter_map(|k| self.actors.get(k))
                .map(|a| a.width + a.margin.unwrap_or(0.0))
                .sum();
            total_width += conf.box_margin * 8.0;
            total_width -= 2.0 * conf.box_text_margin;
            if b.wrap
                && let Some(name) = &b.name
            {
                b.name = Some(wrap(
                    self.metrics,
                    name,
                    total_width - 2.0 * conf.wrap_padding,
                    &conf.fonts.message,
                ));
            }
            let (w, h) = match &b.name {
                Some(name) => text_dims(self.metrics, name, &conf.fonts.message),
                None => (0.0, 0.0),
            };
            max_box_height = max_box_height.max(h);
            let min_width = total_width.max(w + 2.0 * conf.wrap_padding);
            b.margin = conf.box_text_margin;
            if total_width < min_width {
                b.margin += (min_width - total_width) / 2.0;
            }
        }
        for b in &mut self.boxes {
            b.text_max_height = max_box_height;
        }
        max_height.max(conf.height)
    }

    /// `addActorRenderingData` for the top row.
    fn add_actor_rendering_data(&mut self, vertical_pos: f64) {
        let conf = self.conf.clone();
        let mut prev_width = 0.0;
        let mut prev_margin = 0.0;
        let mut prev_box: Option<usize> = None;
        let mut max_height: f64 = 0.0;
        let keys = self.actor_keys.clone();
        for key in &keys {
            let Some(actor) = self.actors.get(key) else {
                continue;
            };
            let b = actor.box_index;
            if let Some(pb) = prev_box
                && Some(pb) != b
            {
                self.model_boxes.push(pb);
                prev_margin += conf.box_margin + self.boxes[pb].margin;
            }
            if let Some(bi) = b
                && Some(bi) != prev_box
            {
                self.boxes[bi].x = prev_width + prev_margin;
                self.boxes[bi].y = vertical_pos;
                prev_margin += self.boxes[bi].margin;
            }
            let created = self.db.created_actors.get(key).copied();
            let starty = self.bounds.vertical_pos;
            let actor = self.actors.get_mut(key).expect("actor present");
            actor.width = (if actor.width != 0.0 {
                actor.width
            } else {
                conf.width
            })
            .max(conf.width);
            actor.height = (if actor.height != 0.0 {
                actor.height
            } else {
                conf.height
            })
            .max(conf.height);
            if actor.margin.is_none() {
                actor.margin = Some(conf.actor_margin);
            }
            max_height = max_height.max(actor.height);
            // `createdActors.get(name)` is the message index; 0 is falsy.
            if created.is_some_and(|i| i != 0) {
                prev_margin += actor.width / 2.0;
            }
            actor.x = prev_width + prev_margin;
            actor.starty = starty;
            let (x, w, h, margin) = (
                actor.x,
                actor.width,
                actor.height,
                actor.margin.unwrap_or(0.0),
            );
            self.bounds.insert(&conf, x, vertical_pos, x + w, h);
            prev_width += w + prev_margin;
            if let Some(bi) = b {
                self.boxes[bi].width = prev_width + self.boxes[bi].margin - self.boxes[bi].x;
            }
            prev_margin = margin;
            prev_box = b;
        }
        if let Some(pb) = prev_box {
            self.model_boxes.push(pb);
        }
        self.bounds.bump_vertical_pos(max_height);
    }

    /// `activationBounds`.
    fn activation_bounds(&self, actor: &str) -> (f64, f64) {
        let center = self.actor_center(actor);
        let mut left = center - 1.0;
        let mut right = center + 1.0;
        for a in self.bounds.activations.iter().filter(|a| a.actor == actor) {
            left = left.min(a.startx);
            right = right.max(a.stopx);
        }
        (left, right)
    }

    /// `buildNoteModel`.
    fn build_note_model(&self, msg: &Message) -> NoteModel {
        let conf = &self.conf;
        let font = &conf.fonts.note;
        let from = msg.from.as_deref().unwrap_or("");
        let to = msg.to.as_deref().unwrap_or("");
        let (fx, fw) = self.actor(from).map_or((0.0, 0.0), |a| (a.x, a.width));
        let (tx, tw) = self.actor(to).map_or((0.0, 0.0), |a| (a.x, a.width));
        let should_wrap = msg.wrap && !msg.message.is_empty();
        let measured = if should_wrap {
            wrap(self.metrics, &msg.message, conf.width, font)
        } else {
            msg.message.clone()
        };
        let (mut tw_text, _) = text_dims(self.metrics, &measured, font);
        let mut model = NoteModel {
            width: if should_wrap {
                conf.width
            } else {
                conf.width.max(tw_text + 2.0 * conf.note_margin)
            },
            height: 0.0,
            startx: fx,
            stopx: 0.0,
            starty: 0.0,
            stopy: 0.0,
            message: msg.message.clone(),
        };
        match msg.placement {
            Some(Placement::RightOf) => {
                model.width = if should_wrap {
                    conf.width.max(tw_text)
                } else {
                    (fw / 2.0 + tw / 2.0).max(tw_text + 2.0 * conf.note_margin)
                };
                model.startx = fx + (fw + conf.actor_margin) / 2.0;
            }
            Some(Placement::LeftOf) => {
                model.width = if should_wrap {
                    conf.width.max(tw_text + 2.0 * conf.note_margin)
                } else {
                    (fw / 2.0 + tw / 2.0).max(tw_text + 2.0 * conf.note_margin)
                };
                model.startx = fx - model.width + (fw - conf.actor_margin) / 2.0;
            }
            _ if from == to => {
                let measured = if should_wrap {
                    wrap(self.metrics, &msg.message, conf.width.max(fw), font)
                } else {
                    msg.message.clone()
                };
                tw_text = text_dims(self.metrics, &measured, font).0;
                model.width = if should_wrap {
                    conf.width.max(fw)
                } else {
                    fw.max(conf.width).max(tw_text + 2.0 * conf.note_margin)
                };
                model.startx = fx + (fw - model.width) / 2.0;
            }
            _ => {
                model.width = (fx + fw / 2.0 - (tx + tw / 2.0)).abs() + conf.actor_margin;
                model.startx = if fx < tx {
                    fx + fw / 2.0 - conf.actor_margin / 2.0
                } else {
                    tx + tw / 2.0 - conf.actor_margin / 2.0
                };
            }
        }
        if should_wrap {
            model.message = wrap(
                self.metrics,
                &msg.message,
                model.width - 2.0 * conf.wrap_padding,
                font,
            );
        }
        model
    }

    /// `calculateCentralConnectionOffset`.
    fn central_connection_offset(msg: &Message, is_arrow_to_right: bool) -> f64 {
        let mut offset = 0.0;
        let reverse_or_dual = matches!(
            msg.central_connection,
            Some(LineType::CentralConnectionReverse | LineType::CentralConnectionDual)
        );
        if reverse_or_dual {
            offset += 4.0;
        }
        if reverse_or_dual && msg.kind.is_some_and(LineType::is_bidirectional) && !is_arrow_to_right
        {
            offset -= 6.0;
        }
        offset
    }

    /// `buildMessageModel`; `None` for the non-message types.
    fn build_message_model(&self, msg: &mut Message) -> Option<MsgModel> {
        let conf = &self.conf;
        let kind = msg.kind.filter(|k| k.is_message())?;
        let from = msg.from.clone().unwrap_or_default();
        let to = msg.to.clone().unwrap_or_default();
        let (from_left, from_right) = self.activation_bounds(&from);
        let (to_left, to_right) = self.activation_bounds(&to);
        let is_arrow_to_right = from_left <= to_left;
        let mut startx = if is_arrow_to_right {
            from_right
        } else {
            from_left
        };
        let mut stopx = if is_arrow_to_right { to_left } else { to_right };
        startx += Renderer::central_connection_offset(msg, is_arrow_to_right);
        let is_arrow_to_activation = (to_left - to_right).abs() > 2.0;
        let adjust = |v: f64| if is_arrow_to_right { -v } else { v };
        if from == to {
            stopx = startx;
        } else {
            if msg.activate && !is_arrow_to_activation {
                stopx += adjust(conf.activation_width / 2.0 - 1.0);
            }
            if !kind.keeps_end() {
                stopx += adjust(3.0);
            }
            if kind.shortens_start() {
                startx -= adjust(3.0);
            }
        }
        let all = [from_left, from_right, to_left, to_right];
        let bounded_width = (startx - stopx).abs();
        if msg.wrap && !msg.message.is_empty() {
            msg.message = wrap(
                self.metrics,
                &msg.message,
                (bounded_width + 2.0 * conf.wrap_padding).max(conf.width),
                &conf.fonts.message,
            );
        }
        let (w, _) = text_dims(self.metrics, &msg.message, &conf.fonts.message);
        Some(MsgModel {
            width: (if msg.wrap {
                0.0
            } else {
                w + 2.0 * conf.wrap_padding
            })
            .max(bounded_width + 2.0 * conf.wrap_padding)
            .max(conf.width),
            height: 0.0,
            startx,
            stopx,
            starty: 0.0,
            stopy: 0.0,
            message: msg.message.clone(),
            kind,
            wrap: msg.wrap,
            from_bounds: all.iter().copied().fold(f64::INFINITY, f64::min),
            to_bounds: all.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            id: msg.id,
            from,
            to,
            sequence_index: 0.0,
            sequence_visible: false,
        })
    }

    /// `calculateLoopBounds`: the width each block needs, and the note and
    /// message models as a side effect.
    fn calculate_loop_bounds(&mut self) {
        let conf = self.conf.clone();
        struct Open {
            id: usize,
            from: f64,
            to: f64,
            width: f64,
        }
        let mut stack: Vec<Open> = Vec::new();
        let mut loops: HashMap<usize, LoopWidth> = HashMap::new();
        let store = |loops: &mut HashMap<usize, LoopWidth>, id: usize, cur: &Open| {
            loops.insert(
                id,
                LoopWidth {
                    from: cur.from,
                    to: cur.to,
                    width: cur.width,
                },
            );
        };
        let mut messages = std::mem::take(&mut self.messages);
        for msg in &mut messages {
            match msg.kind {
                Some(
                    LineType::LoopStart
                    | LineType::AltStart
                    | LineType::OptStart
                    | LineType::ParStart
                    | LineType::ParOverStart
                    | LineType::CriticalStart
                    | LineType::BreakStart,
                ) => stack.push(Open {
                    id: msg.id,
                    from: f64::MAX,
                    to: f64::MIN,
                    width: 0.0,
                }),
                Some(LineType::AltElse | LineType::ParAnd | LineType::CriticalOption) => {
                    if !msg.message.is_empty()
                        && let Some(cur) = stack.last()
                    {
                        store(&mut loops, cur.id, cur);
                        store(&mut loops, msg.id, cur);
                    }
                }
                Some(
                    LineType::LoopEnd
                    | LineType::AltEnd
                    | LineType::OptEnd
                    | LineType::ParEnd
                    | LineType::CriticalEnd
                    | LineType::BreakEnd,
                ) => {
                    if let Some(cur) = stack.pop() {
                        store(&mut loops, cur.id, &cur);
                    }
                }
                Some(LineType::ActiveStart) => {
                    let from = msg.from.clone().unwrap_or_default();
                    let stacked = self.bounds.actor_activations(&from) as f64;
                    let x =
                        self.actor_center(&from) + ((stacked - 1.0) * conf.activation_width) / 2.0;
                    self.bounds.activations.push(Activation {
                        startx: x,
                        starty: 0.0,
                        stopx: x + conf.activation_width,
                        stopy: None,
                        actor: from,
                        anchored: 0,
                    });
                }
                Some(LineType::ActiveEnd) => {
                    let from = msg.from.as_deref().unwrap_or("");
                    if let Some(idx) = self
                        .bounds
                        .activations
                        .iter()
                        .rposition(|a| a.actor == from)
                    {
                        self.bounds.activations.remove(idx);
                    }
                }
                _ => {}
            }
            if msg.placement.is_some() {
                let note = self.build_note_model(msg);
                for cur in &mut stack {
                    cur.from = cur.from.min(note.startx);
                    cur.to = cur.to.max(note.startx + note.width);
                    cur.width = cur.width.max((cur.from - cur.to).abs()) - conf.label_box_width;
                }
                self.note_models.insert(msg.id, note);
            } else if let Some(model) = self.build_message_model(msg) {
                // `if (msgModel.startx && msgModel.stopx)`: zero is falsy.
                if model.startx != 0.0 && model.stopx != 0.0 && !stack.is_empty() {
                    for cur in &mut stack {
                        if model.startx == model.stopx {
                            let f = self
                                .actor(&model.from)
                                .map_or((0.0, 0.0), |a| (a.x, a.width));
                            let t = self.actor(&model.to).map_or(0.0, |a| a.x);
                            cur.from = cur.from.min(f.0 - model.width / 2.0).min(f.0 - f.1 / 2.0);
                            cur.to = cur.to.max(t + model.width / 2.0).max(t + f.1 / 2.0);
                            cur.width =
                                cur.width.max((cur.to - cur.from).abs()) - conf.label_box_width;
                        } else {
                            cur.from = cur.from.min(model.startx);
                            cur.to = cur.to.max(model.stopx);
                            cur.width = cur.width.max(model.width) - conf.label_box_width;
                        }
                    }
                }
                self.msg_models.insert(msg.id, model);
            }
        }
        self.messages = messages;
        self.bounds.activations.clear();
        self.loop_widths = loops;
    }

    /// `drawNote`.
    fn draw_note(&mut self, id: usize) {
        let conf = self.conf.clone();
        let Some(mut note) = self.note_models.remove(&id) else {
            return;
        };
        self.bounds.bump_vertical_pos(conf.box_margin);
        note.height = conf.box_margin;
        note.starty = self.bounds.vertical_pos;
        let width = if note.width != 0.0 {
            note.width
        } else {
            conf.width
        };
        let data = TextData {
            x: note.startx,
            y: note.starty,
            width: Some(width),
            text: note.message.clone(),
            class: "noteText",
            anchor: Some(conf.note_align.clone()),
            valign: Some("center"),
            text_margin: conf.note_margin,
            dy_em: true,
            tspan: true,
        };
        let ctx = self.ctx();
        let (texts, heights) =
            draw::draw_text(&ctx, &data, &conf.fonts.note, self.styles.note_text());
        let text_height = js_round(heights.iter().sum());
        let rect_height = text_height + 2.0 * conf.note_margin;
        let mut children = vec![draw::rect(
            note.startx,
            note.starty,
            width,
            rect_height,
            None,
            None,
            "note",
            self.styles.note_rect(),
        )];
        children.extend(texts);
        self.main.push(Element::Group(Group {
            common: Common::default(),
            children,
        }));
        note.height += rect_height;
        self.bounds.bump_vertical_pos(rect_height);
        note.stopy = note.starty + rect_height;
        note.stopx = note.startx + width;
        self.bounds
            .insert(&conf, note.startx, note.starty, note.stopx, note.stopy);
        self.note_models.insert(id, note);
    }

    /// `boundMessage`: reserves the message's rows and returns `lineStartY`.
    fn bound_message(&mut self, model: &mut MsgModel) -> f64 {
        let conf = self.conf.clone();
        self.bounds.bump_vertical_pos(10.0);
        let lines = split_breaks(&model.message).len() as f64;
        let (text_width, text_height) =
            text_dims(self.metrics, &model.message, &conf.fonts.message);
        let line_height = text_height / lines;
        model.height += line_height;
        self.bounds.bump_vertical_pos(line_height);
        let mut total_offset = text_height - 10.0;
        let line_start_y;
        if model.startx == model.stopx {
            line_start_y = if conf.right_angles {
                self.bounds.vertical_pos + total_offset
            } else {
                total_offset += conf.box_margin;
                self.bounds.vertical_pos + total_offset
            };
            total_offset += 30.0;
            let dx = (text_width / 2.0).max(conf.width / 2.0);
            let vp = self.bounds.vertical_pos;
            self.bounds.insert(
                &conf,
                model.startx - dx,
                vp - 10.0 + total_offset,
                model.stopx + dx,
                vp + 30.0 + total_offset,
            );
        } else {
            total_offset += conf.box_margin;
            line_start_y = self.bounds.vertical_pos + total_offset;
            self.bounds.insert(
                &conf,
                model.startx,
                line_start_y - 10.0,
                model.stopx,
                line_start_y,
            );
        }
        self.bounds.bump_vertical_pos(total_offset);
        model.height += total_offset;
        model.stopy = model.starty + model.height;
        self.bounds.insert(
            &conf,
            model.from_bounds,
            model.starty,
            model.to_bounds,
            model.stopy,
        );
        line_start_y
    }

    /// `adjustLoopHeightForWrap`.
    fn adjust_loop_height_for_wrap(
        &mut self,
        index: usize,
        pre_margin: f64,
        post_margin: f64,
        add: impl FnOnce(&mut Bounds, &Message),
    ) {
        let conf = self.conf.clone();
        self.bounds.bump_vertical_pos(pre_margin);
        let mut height_adjust = post_margin;
        let mut msg = self.messages[index].clone();
        if !msg.message.is_empty()
            && let Some(lw) = self.loop_widths.get(&msg.id)
        {
            let loop_width = lw.width;
            msg.message = wrap(
                self.metrics,
                &format!("[{}]", msg.message),
                loop_width - 2.0 * conf.wrap_padding,
                &conf.fonts.message,
            );
            msg.wrap = true;
            self.msg_widths.insert(msg.id, loop_width);
            let (_, h) = text_dims(self.metrics, &msg.message, &conf.fonts.message);
            let total_offset = h.max(conf.label_box_height);
            height_adjust = post_margin + total_offset;
        }
        add(&mut self.bounds, &msg);
        self.messages[index] = msg;
        self.bounds.bump_vertical_pos(height_adjust);
    }

    /// `adjustCreatedDestroyedData`.
    fn adjust_created_destroyed(&mut self, index: usize, model: &mut MsgModel, line_start_y: f64) {
        let conf = self.conf.clone();
        let is_icon = |kind: &str| matches!(kind, "actor" | "control" | "entity" | "database");
        let created = self.db.created_actors.get(&model.to).copied() == Some(index);
        let destroyed_from = self.db.destroyed_actors.get(&model.from).copied() == Some(index);
        let destroyed_to = self.db.destroyed_actors.get(&model.to).copied() == Some(index);
        let from_x = self.actor(&model.from).map_or(0.0, |a| a.x);
        let to_x = self.actor(&model.to).map_or(0.0, |a| a.x);

        if created {
            let Some(actor) = self.actor(&model.to).cloned() else {
                return;
            };
            let adjustment = if is_icon(&actor.kind) {
                ACTOR_TYPE_WIDTH / 2.0 + 3.0
            } else {
                actor.width / 2.0 + 3.0
            };
            self.receiver_adjustment(model, &actor, from_x, adjustment);
            if let Some(a) = self.actors.get_mut(&model.to) {
                a.starty = line_start_y - a.height / 2.0;
            }
            self.bounds.bump_vertical_pos(actor.height / 2.0);
        } else if destroyed_from {
            let Some(actor) = self.actor(&model.from).cloned() else {
                return;
            };
            if conf.mirror_actors {
                let adjustment = if is_icon(&actor.kind) {
                    ACTOR_TYPE_WIDTH / 2.0
                } else {
                    actor.width / 2.0
                };
                self.sender_adjustment(model, &actor, to_x, adjustment);
            }
            if let Some(a) = self.actors.get_mut(&model.from) {
                a.stopy = Some(line_start_y - a.height / 2.0);
            }
            self.bounds.bump_vertical_pos(actor.height / 2.0);
        } else if destroyed_to {
            let Some(actor) = self.actor(&model.to).cloned() else {
                return;
            };
            if conf.mirror_actors {
                let adjustment = if is_icon(&actor.kind) {
                    ACTOR_TYPE_WIDTH / 2.0 + 3.0
                } else {
                    actor.width / 2.0 + 3.0
                };
                self.receiver_adjustment(model, &actor, from_x, adjustment);
            }
            if let Some(a) = self.actors.get_mut(&model.to) {
                a.stopy = Some(line_start_y - a.height / 2.0);
            }
            self.bounds.bump_vertical_pos(actor.height / 2.0);
        }
    }

    fn receiver_adjustment(
        &mut self,
        model: &mut MsgModel,
        actor: &ActorModel,
        from_x: f64,
        adjustment: f64,
    ) {
        let conf = self.conf.clone();
        if actor.x < from_x {
            self.bounds.insert(
                &conf,
                model.stopx - adjustment,
                model.starty,
                model.startx,
                model.stopy + actor.height / 2.0 + conf.note_margin,
            );
            model.stopx += adjustment;
        } else {
            self.bounds.insert(
                &conf,
                model.startx,
                model.starty,
                model.stopx + adjustment,
                model.stopy + actor.height / 2.0 + conf.note_margin,
            );
            model.stopx -= adjustment;
        }
    }

    fn sender_adjustment(
        &mut self,
        model: &mut MsgModel,
        actor: &ActorModel,
        to_x: f64,
        adjustment: f64,
    ) {
        let conf = self.conf.clone();
        if actor.x < to_x {
            self.bounds.insert(
                &conf,
                model.startx - adjustment,
                model.starty,
                model.stopx,
                model.stopy + actor.height / 2.0 + conf.note_margin,
            );
            model.startx += adjustment;
        } else {
            self.bounds.insert(
                &conf,
                model.stopx,
                model.starty,
                model.startx + adjustment,
                model.stopy + actor.height / 2.0 + conf.note_margin,
            );
            model.startx -= adjustment;
        }
    }

    /// `activeEnd` inside `draw`.
    fn active_end(&mut self, msg: &Message) {
        let conf = self.conf.clone();
        let from = msg.from.as_deref().unwrap_or("");
        let Some(idx) = self
            .bounds
            .activations
            .iter()
            .rposition(|a| a.actor == from)
        else {
            return;
        };
        let mut activation = self.bounds.activations.remove(idx);
        let mut vertical_pos = self.bounds.vertical_pos;
        if activation.starty + 18.0 > vertical_pos {
            activation.starty = vertical_pos - 6.0;
            vertical_pos += 12.0;
        }
        let count = self.bounds.actor_activations(from);
        let rect = draw::draw_activation(&self.ctx(), &activation, vertical_pos, count);
        if let Some(Element::Group(g)) = self.main.get_mut(activation.anchored) {
            g.children.push(rect);
        }
        self.bounds.insert(
            &conf,
            activation.startx,
            vertical_pos - 10.0,
            activation.stopx,
            vertical_pos,
        );
    }

    fn new_activation(&mut self, msg: &Message) {
        let conf = &self.conf;
        let from = msg.from.clone().unwrap_or_default();
        let stacked = self.bounds.actor_activations(&from) as f64;
        let x = self.actor_center(&from) + ((stacked - 1.0) * conf.activation_width) / 2.0;
        self.main.push(Element::Group(Group::default()));
        self.bounds.activations.push(Activation {
            startx: x,
            starty: self.bounds.vertical_pos + 2.0,
            stopx: x + conf.activation_width,
            stopy: None,
            actor: from,
            anchored: self.main.len() - 1,
        });
    }

    fn end_block(&mut self, label: Option<&str>) {
        let Some(mut model) = self.bounds.end_loop() else {
            return;
        };
        if let Some(label) = label {
            let g = draw::draw_loop(&self.ctx(), &mut model, label);
            self.main.push(Element::Group(g));
        }
        let stopy = model.stopy.unwrap_or(model.starty);
        self.bounds
            .bump_vertical_pos(stopy - self.bounds.vertical_pos);
        if label.is_none() {
            self.backgrounds.push(model);
        }
    }

    /// `drawActors` for one row. Returns the max height a footer drew.
    fn draw_actors(&mut self, is_footer: bool) -> f64 {
        let conf = self.conf.clone();
        let mut max_height: f64 = 0.0;
        if is_footer {
            self.bounds.bump_vertical_pos(conf.box_margin * 2.0);
        }
        let keys = self.actor_keys.clone();
        for key in &keys {
            let Some(mut actor) = self.actors.get(key).cloned() else {
                continue;
            };
            if is_footer && actor.stopy.is_none() {
                actor.stopy = Some(self.bounds.vertical_pos);
            }
            if !is_footer {
                actor.actor_cnt = Some(self.actor_cnt);
                self.actor_cnt += 1;
            }
            let ctx = self.ctx();
            let drawn = if actor.kind == "actor" {
                draw::draw_actor_actor(&ctx, &mut actor, is_footer)
            } else {
                draw::draw_actor_participant(&ctx, &mut actor, is_footer)
            };
            max_height = max_height.max(drawn.height);
            self.front.push(Element::Group(drawn.lowered));
            if !is_footer {
                self.lifelines.insert(key.clone(), self.front.len() - 1);
            }
            if let Some(g) = drawn.appended {
                self.main.push(Element::Group(g));
            }
            self.actors.insert(key.clone(), actor);
        }
        if is_footer {
            self.bounds.bump_vertical_pos(max_height + conf.box_margin);
        }
        max_height
    }

    /// `fixLifeLineHeights`: `y2` of every top lifeline.
    fn fix_life_line_heights(&mut self) {
        let mirror = self.conf.mirror_actors;
        for key in &self.actor_keys {
            let (Some(actor), Some(&idx)) = (self.actors.get(key), self.lifelines.get(key)) else {
                continue;
            };
            let y2 = match (mirror, actor.stopy) {
                (false, Some(stopy)) => Some(stopy + actor.height / 2.0),
                (true, stopy) => stopy,
                _ => None,
            };
            if let (Some(y2), Some(Element::Group(g))) = (y2, self.front.get_mut(idx))
                && let Some(Element::Line(l)) = g.children.first_mut()
            {
                l.y2 = y2;
            }
        }
    }

    /// `drawMessage`.
    fn draw_message(&mut self, model: &MsgModel, line_start_y: f64, msg: &Message) {
        let conf = self.conf.clone();
        let styles = self.styles;
        let (startx, stopx, starty) = (model.startx, model.stopx, model.starty);
        let (text_width, _) = text_dims(self.metrics, &model.message, &conf.fonts.message);
        let data = TextData {
            x: startx.min(stopx),
            y: starty + 10.0,
            width: Some((stopx - startx).abs()),
            text: model.message.clone(),
            class: "messageText",
            anchor: Some(conf.message_align.clone()),
            valign: Some("center"),
            text_margin: conf.wrap_padding,
            dy_em: true,
            tspan: false,
        };
        let (texts, _) = draw::draw_text(
            &self.ctx(),
            &data,
            &conf.fonts.message,
            styles.message_text(),
        );
        self.main.extend(texts);

        let kind = model.kind;
        let mut style = styles.message_line(kind.is_dotted());
        let class = if kind.is_dotted() {
            "messageLine1"
        } else {
            "messageLine0"
        };
        let marker = |name: &str| Some(draw::marker_ref(name));
        match kind {
            LineType::SolidTop | LineType::SolidTopDotted => {
                style.marker_end = marker("solidTopArrowHead")
            }
            LineType::SolidBottom | LineType::SolidBottomDotted => {
                style.marker_end = marker("solidBottomArrowHead")
            }
            LineType::StickTop | LineType::StickTopDotted => {
                style.marker_end = marker("stickTopArrowHead")
            }
            LineType::StickBottom | LineType::StickBottomDotted => {
                style.marker_end = marker("stickBottomArrowHead")
            }
            LineType::SolidArrowTopReverse | LineType::SolidArrowTopReverseDotted => {
                style.marker_start = marker("solidBottomArrowHead")
            }
            LineType::SolidArrowBottomReverse | LineType::SolidArrowBottomReverseDotted => {
                style.marker_start = marker("solidTopArrowHead")
            }
            LineType::StickArrowTopReverse | LineType::StickArrowTopReverseDotted => {
                style.marker_start = marker("stickBottomArrowHead")
            }
            LineType::StickArrowBottomReverse | LineType::StickArrowBottomReverseDotted => {
                style.marker_start = marker("stickTopArrowHead")
            }
            LineType::Solid | LineType::Dotted => style.marker_end = marker("arrowhead"),
            LineType::BidirectionalSolid | LineType::BidirectionalDotted => {
                style.marker_start = marker("arrowhead");
                style.marker_end = marker("arrowhead");
            }
            LineType::SolidPoint | LineType::DottedPoint => {
                style.marker_end = marker("filled-head")
            }
            LineType::SolidCross | LineType::DottedCross => style.marker_end = marker("crosshead"),
            _ => {}
        }

        let numbers_on = model.sequence_visible || conf.show_sequence_numbers;
        let has_central = msg
            .central_connection
            .is_some_and(LineType::is_central_connection);
        let common = Common {
            class: Some(class.to_string()),
            style,
            ..Common::default()
        };
        if startx == stopx {
            let line_start_x = startx
                + if numbers_on && (kind.is_reverse() || kind.is_bidirectional()) {
                    10.0
                } else {
                    0.0
                };
            let d = if conf.right_angles {
                draw::self_message_right_angles(
                    line_start_x,
                    startx,
                    startx + (conf.width / 2.0).max(text_width / 2.0),
                    line_start_y,
                )
            } else {
                draw::self_message_curve(line_start_x, startx, line_start_y)
            };
            self.main.push(Element::Path(Path { common, d }));
            if has_central {
                self.draw_central_connection(msg, model, line_start_y);
            }
        } else {
            let mut x1 = startx;
            let mut x2 = stopx;
            if numbers_on {
                let radius = 6.0;
                let reverse_or_dual = matches!(
                    msg.central_connection,
                    Some(LineType::CentralConnectionDual | LineType::CentralConnectionReverse)
                );
                if kind.is_bidirectional() {
                    x1 = if startx < stopx {
                        startx + radius * 2.0
                    } else {
                        startx - radius
                            + if has_central { -5.0 } else { 0.0 }
                            + if reverse_or_dual { -7.5 } else { 0.0 }
                    };
                } else if kind.is_reverse() {
                    if stopx > startx {
                        x2 = stopx - 2.0 * radius;
                    } else {
                        x2 = stopx - radius;
                        x1 += if reverse_or_dual { -7.5 } else { 0.0 };
                    }
                    x2 += if has_central { 15.0 } else { 0.0 };
                } else {
                    x1 = startx + radius;
                }
            }
            self.main.push(Element::Line(Line {
                common,
                x1,
                y1: line_start_y,
                x2,
                y2: line_start_y,
            }));
            if has_central {
                self.draw_central_connection(msg, model, line_start_y);
            }
        }

        if numbers_on {
            let is_self = startx == stopx;
            let left_to_right = startx <= stopx;
            let autonumber_x = if is_self {
                model.from_bounds + 1.0
            } else if kind.is_reverse() {
                if left_to_right {
                    model.to_bounds - 1.0
                } else {
                    model.from_bounds + 1.0
                }
            } else if left_to_right {
                model.from_bounds + 1.0
            } else {
                model.to_bounds - 1.0
            };
            let index_text = crate::mermaid::theme::khroma::js_number(model.sequence_index);
            let font_size = match index_text.len() {
                n if n > 5 => 7.0,
                n if n > 3 => 9.0,
                _ => 12.0,
            };
            let mut line_style = styles.sequence_number_line();
            line_style.marker_start = marker("sequencenumber");
            self.main.push(Element::Line(Line {
                common: Common::with_style(line_style),
                x1: autonumber_x,
                y1: line_start_y,
                x2: autonumber_x,
                y2: line_start_y,
            }));
            let mut text_style = styles.sequence_number_text();
            text_style.font.family = Some("sans-serif".to_string());
            text_style.font.size = Some(font_size);
            self.main.push(Element::Text(Text {
                common: Common {
                    class: Some("sequenceNumber".to_string()),
                    style: text_style,
                    ..Common::default()
                },
                x: autonumber_x,
                y: line_start_y + 4.0,
                anchor: Some(TextAnchor::Middle),
                baseline: None,
                font: Default::default(),
                runs: vec![TextRun::new(index_text)],
            }));
        }
    }

    /// `drawCentralConnection`: the circles at the connected ends.
    fn draw_central_connection(&mut self, msg: &Message, model: &MsgModel, line_start_y: f64) {
        let mut from_center = self.actor_center(&model.from);
        let mut to_center = self.actor_center(&model.to);
        let left_to_right = from_center <= to_center;
        let is_reverse = msg.kind.is_some_and(LineType::is_reverse);
        let offset = |reverse: bool| {
            let base = if left_to_right { 16.5 } else { -16.5 };
            if reverse { -base } else { base }
        };
        if model.sequence_visible {
            match msg.central_connection {
                Some(LineType::CentralConnection) if is_reverse => to_center += offset(true),
                Some(LineType::CentralConnectionReverse) if !is_reverse => {
                    from_center += offset(false)
                }
                Some(LineType::CentralConnectionDual) => {
                    if is_reverse {
                        to_center += offset(true);
                    } else {
                        from_center += offset(false);
                    }
                }
                _ => {}
            }
        }
        let circle = |cx: f64| {
            Element::Circle(Circle {
                common: Common::with_style(self.styles.root_fill()),
                cx,
                cy: line_start_y,
                r: 5.0,
            })
        };
        let mut children = Vec::new();
        match msg.central_connection {
            Some(LineType::CentralConnection) => children.push(circle(to_center)),
            Some(LineType::CentralConnectionReverse) => children.push(circle(from_center)),
            Some(LineType::CentralConnectionDual) => {
                children.push(circle(from_center));
                children.push(circle(to_center));
            }
            _ => {}
        }
        self.main.push(Element::Group(Group {
            common: Common::default(),
            children,
        }));
    }
}

impl Renderer<'_> {
    /// `draw`, from the first measurement to the viewBox. Returns the
    /// root children in paint order and the viewBox as mermaid writes it:
    /// `(x, y, width, height)`.
    pub fn draw(mut self) -> (Vec<Element>, [f64; 4]) {
        let widths = self.max_message_width_per_actor();
        self.conf.height = self.calculate_actor_margins(&widths);
        let conf = self.conf.clone();
        let has_boxes = self.db.has_at_least_one_box();
        let has_box_titles = self.db.has_at_least_one_box_with_title();

        if has_boxes {
            self.bounds.bump_vertical_pos(conf.box_margin);
            if has_box_titles {
                let h = self.boxes.first().map_or(0.0, |b| b.text_max_height);
                self.bounds.bump_vertical_pos(h);
            }
        }
        if conf.hide_unused_participants {
            let used: std::collections::HashSet<String> = self
                .messages
                .iter()
                .flat_map(|m| [m.from.clone(), m.to.clone()])
                .flatten()
                .collect();
            self.actor_keys.retain(|k| used.contains(k));
        }
        self.add_actor_rendering_data(0.0);
        self.calculate_loop_bounds();

        let mut sequence_index = 1.0;
        let mut sequence_index_step = 1.0;
        let mut sequence_numbers = self.db.sequence_numbers_enabled;
        let mut messages_to_draw: Vec<(MsgModel, f64, Message)> = Vec::new();
        let count = self.messages.len();
        for index in 0..count {
            let msg = self.messages[index].clone();
            let bm = conf.box_margin;
            let btm = conf.box_text_margin;
            match msg.kind {
                Some(LineType::Note) => self.draw_note(msg.id),
                Some(
                    LineType::ActiveStart
                    | LineType::CentralConnection
                    | LineType::CentralConnectionReverse,
                ) => self.new_activation(&msg),
                Some(LineType::ActiveEnd) => self.active_end(&msg),
                Some(
                    LineType::LoopStart
                    | LineType::OptStart
                    | LineType::AltStart
                    | LineType::CriticalStart
                    | LineType::BreakStart
                    | LineType::ParStart
                    | LineType::ParOverStart,
                ) => self.adjust_loop_height_for_wrap(index, bm, bm + btm, |b, m| {
                    b.new_loop(Some(m), None)
                }),
                Some(LineType::RectStart) => {
                    self.adjust_loop_height_for_wrap(index, bm, bm, |b, m| {
                        let fill = if m.message.is_empty() {
                            None
                        } else {
                            Some(m.message.clone())
                        };
                        b.new_loop(None, fill);
                    })
                }
                Some(LineType::AltElse | LineType::ParAnd | LineType::CriticalOption) => self
                    .adjust_loop_height_for_wrap(index, bm + btm, bm, |b, m| {
                        b.add_section_to_loop(m)
                    }),
                Some(LineType::LoopEnd) => self.end_block(Some("loop")),
                Some(LineType::OptEnd) => self.end_block(Some("opt")),
                Some(LineType::AltEnd) => self.end_block(Some("alt")),
                Some(LineType::ParEnd) => self.end_block(Some("par")),
                Some(LineType::CriticalEnd) => self.end_block(Some("critical")),
                Some(LineType::BreakEnd) => self.end_block(Some("break")),
                Some(LineType::RectEnd) => self.end_block(None),
                Some(LineType::Autonumber) => {
                    if let Some(a) = msg.autonumber {
                        if let Some(s) = a.start.filter(|v| *v != 0.0) {
                            sequence_index = s;
                        }
                        if let Some(s) = a.step.filter(|v| *v != 0.0) {
                            sequence_index_step = s;
                        }
                        sequence_numbers = a.visible;
                    }
                }
                _ => {
                    if let Some(mut model) = self.msg_models.remove(&msg.id) {
                        model.starty = self.bounds.vertical_pos;
                        model.sequence_index = sequence_index;
                        model.sequence_visible = sequence_numbers;
                        let line_start_y = self.bound_message(&mut model);
                        self.adjust_created_destroyed(index, &mut model, line_start_y);
                        messages_to_draw.push((model, line_start_y, msg.clone()));
                    }
                }
            }
            if msg.kind.is_some_and(LineType::is_message) {
                sequence_index = ((sequence_index + sequence_index_step) * 100.0).round() / 100.0;
            }
        }

        self.draw_actors(false);
        for (model, line_start_y, msg) in &messages_to_draw {
            self.draw_message(model, *line_start_y, msg);
        }
        if conf.mirror_actors {
            self.draw_actors(true);
        }
        let backgrounds = std::mem::take(&mut self.backgrounds);
        for model in &backgrounds {
            let el = draw::draw_background_rect(&self.ctx(), model);
            self.front.push(el);
        }
        self.fix_life_line_heights();

        let model_boxes = self.model_boxes.clone();
        for bi in model_boxes {
            let vp = self.bounds.vertical_pos;
            let b = &mut self.boxes[bi];
            b.height = vp - b.y;
            let (x, y, w, h) = (b.x, b.y, b.width, b.height);
            self.bounds.insert(&conf, x, y, x + w, h);
            let b = &mut self.boxes[bi];
            let padding = conf.box_margin * 2.0;
            b.startx = b.x - padding;
            b.starty = b.y - padding * 0.25;
            b.stopx = b.startx + b.width + 2.0 * padding;
            b.stopy = b.starty + b.height + padding * 0.75;
            let group = draw::draw_box(&self.ctx(), &self.boxes[bi]);
            self.front.push(Element::Group(group));
        }
        if has_boxes {
            self.bounds.bump_vertical_pos(conf.box_margin);
        }

        let data = self.bounds.data.clone();
        let startx = data.startx.unwrap_or(0.0);
        let starty = data.starty.unwrap_or(0.0);
        let stopx = data.stopx.unwrap_or(0.0);
        let stopy = data.stopy.unwrap_or(0.0);
        let box_height = stopy - starty;
        let mut height = box_height + 2.0 * conf.diagram_margin_y;
        if conf.mirror_actors {
            height = height - conf.box_margin + conf.bottom_margin_adj;
        }
        let width = stopx - startx + 2.0 * conf.diagram_margin_x;

        let title = self.db.title.clone().filter(|t| !t.is_empty());
        if let Some(title) = &title {
            self.main.push(Element::Text(Text {
                common: Common::with_style(self.styles.root_fill()),
                x: (stopx - startx) / 2.0 - 2.0 * conf.diagram_margin_x,
                y: -25.0,
                anchor: None,
                baseline: None,
                font: Default::default(),
                runs: vec![TextRun::new(title.clone())],
            }));
        }
        let extra_vert_for_title = if title.is_some() { 40.0 } else { 0.0 };
        let view_box = [
            startx - conf.diagram_margin_x,
            -(conf.diagram_margin_y + extra_vert_for_title),
            width,
            height + extra_vert_for_title,
        ];

        let mut root: Vec<Element> = self.front;
        root.reverse();
        root.extend(self.main);
        (root, view_box)
    }
}
