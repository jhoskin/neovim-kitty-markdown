//! Port of `usecase.visitor.ts` and `usecaseModelBuilder.ts`: the
//! statements become drafts, and `finalize` resolves them into the
//! committed model of `usecaseDb.ts`, with the builder's semantic errors.
//!
//! The public AST (`usecaseAst.ts`) is not built: nothing the renderer
//! reads comes from it.

use std::collections::HashMap;

use indexmap::IndexMap;

use super::db::{
    Actor, ActorType, Boundary, BoundaryType, JsonNode, Note, Relationship, UseCase, UsecaseDb,
};
use super::json::parse_ordered_object;
use super::lexer::Positions;
use super::parser::{
    Arrow, ArrowType, BoundaryChild, Entity, Label, MetaProp, MetaValue, Metadata, RelationKind,
    RelationTail, Shape, Span, Statement,
};

/// A builder or visitor error: the full message mermaid throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError {
    pub message: String,
    /// The first line the message names.
    pub line: usize,
}

type MResult<T> = Result<T, ModelError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Actor,
    Usecase,
    Boundary,
    Json,
    Edge,
}

impl SymbolKind {
    fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Actor => "actor",
            SymbolKind::Usecase => "usecase",
            SymbolKind::Boundary => "boundary",
            SymbolKind::Json => "json",
            SymbolKind::Edge => "edge",
        }
    }
}

#[derive(Debug, Clone)]
struct SymbolOrigin {
    kind: SymbolKind,
    location: Span,
    generated: bool,
    label: Option<String>,
}

#[derive(Debug, Clone)]
struct ElementDraft {
    id: String,
    kind: SymbolKind,
    label: Label,
    location: Span,
    generated: bool,
    parent: Option<(String, Span)>,
    shape: Option<Shape>,
    metadata: Option<Metadata>,
    stereotype: Option<String>,
    classes: Vec<String>,
}

#[derive(Debug, Clone)]
struct BoundaryDraft {
    id: String,
    label: Label,
    location: Span,
    generated: bool,
    classes: Vec<String>,
}

#[derive(Debug, Clone)]
struct JsonDraft {
    id: String,
    value: IndexMap<String, super::json::JsonValue>,
    location: Span,
    classes: Vec<String>,
}

#[derive(Debug, Clone)]
struct Endpoint {
    id: String,
    label: Label,
    location: Span,
    generated: bool,
    declaration: bool,
    classes_on_reference: bool,
}

#[derive(Debug, Clone)]
struct RelationshipDraft {
    source: Endpoint,
    target: Endpoint,
    location: Span,
    explicit_id: Option<(String, Span)>,
    arrow: Arrow,
}

#[derive(Debug, Clone)]
struct NoteDraft {
    target: String,
    target_location: Span,
    label: Label,
}

#[derive(Debug, Clone)]
struct ElementState {
    kind: SymbolKind,
    id: String,
    label: Label,
    location: Span,
    parent: Option<(String, Span)>,
    shape: Option<Shape>,
    stereotype: Option<String>,
    stereotype_location: Option<Span>,
    classes: Vec<String>,
    actor_type: Option<ActorType>,
    icon: Option<String>,
    business: Option<bool>,
}

#[derive(Debug, Clone)]
struct BoundaryState {
    label: Label,
    location: Span,
    boundary_type: Option<BoundaryType>,
    classes: Vec<String>,
    members: Vec<String>,
}

/// `DraftClassAssignment`: the targets and the classes.
type ClassAssignment = (Vec<(String, Span)>, Vec<String>);

fn push_unique(target: &mut Vec<String>, values: &[String]) {
    for v in values {
        if !target.contains(v) {
            target.push(v.clone());
        }
    }
}

/// The builder state for one parse.
struct Builder<'a> {
    text: &'a str,
    positions: Positions<'a>,
    db: UsecaseDb,
    elements: Vec<ElementDraft>,
    boundaries: Vec<BoundaryDraft>,
    json: Vec<JsonDraft>,
    relationships: Vec<RelationshipDraft>,
    notes: Vec<NoteDraft>,
    metadata_assignments: Vec<(String, Span, Metadata)>,
    class_assignments: Vec<ClassAssignment>,
    style_assignments: Vec<(String, Span, Vec<String>)>,
    class_defs: Vec<(Vec<String>, Vec<String>)>,
    directions: Vec<String>,
    parent: Option<(String, Span)>,
}

impl<'a> Builder<'a> {
    /// `locationText(location)`: `line L, column C [s,e)`.
    fn location_text(&self, span: Span) -> String {
        let (line, column) = self.positions.line_column(span.0);
        format!(
            "line {line}, column {column} [{},{})",
            self.positions.offset(span.0),
            self.positions.offset(span.1)
        )
    }

    fn fail(&self, message: String, span: Span) -> ModelError {
        ModelError {
            message,
            line: self.positions.line_column(span.0).0,
        }
    }

    /// `conflict(message, current, previous, currentLabel, previousLabel)`.
    fn conflict(
        &self,
        message: &str,
        current: Span,
        previous: Span,
        current_label: Option<&str>,
        previous_label: Option<&str>,
    ) -> ModelError {
        let suffix = |l: Option<&str>| l.map_or(String::new(), |l| format!(" (label \"{l}\")"));
        self.fail(
            format!(
                "{message} at {}{}; previous declaration at {}{}",
                self.location_text(current),
                suffix(current_label),
                self.location_text(previous),
                suffix(previous_label)
            ),
            current,
        )
    }

    fn invalid_metadata(&self, id: &str, kind: &str, property: &MetaProp) -> ModelError {
        self.fail(
            format!(
                "Metadata property '{}' is invalid for {kind} '{id}' at {}",
                property.key,
                self.location_text(property.loc)
            ),
            property.loc,
        )
    }

    // -- visitor ------------------------------------------------------

    fn element_draft(&self, entity: &Entity, kind: SymbolKind) -> ElementDraft {
        ElementDraft {
            id: entity.id.clone(),
            kind,
            label: entity.label.clone(),
            location: entity.loc,
            generated: entity.generated,
            parent: self.parent.clone(),
            shape: if kind == SymbolKind::Usecase {
                entity.shape
            } else {
                None
            },
            metadata: entity.metadata.clone(),
            stereotype: entity.stereotype.clone(),
            classes: entity.classes.clone(),
        }
    }

    fn endpoint(entity: &Entity, declaration: bool) -> Endpoint {
        Endpoint {
            id: entity.id.clone(),
            label: entity.label.clone(),
            location: entity.loc,
            generated: entity.generated,
            declaration,
            classes_on_reference: !entity.classes.is_empty(),
        }
    }

    fn add_relationship(&mut self, source: Endpoint, tail: &RelationTail) {
        self.relationships.push(RelationshipDraft {
            source,
            target: Self::endpoint(&tail.target, tail.target.explicit_declaration),
            location: tail.span,
            explicit_id: tail.explicit_id.clone(),
            arrow: tail.arrow.clone(),
        });
    }

    fn visit(&mut self, statement: &Statement) -> MResult<()> {
        match statement {
            Statement::AccTitle(t) => self.db.acc_title = t.clone(),
            Statement::AccDescr(d) => self.db.acc_description = d.clone(),
            Statement::Direction(d) => self.directions.push(if d == "TD" {
                "TB".to_string()
            } else {
                d.clone()
            }),
            Statement::Actor { items, relation } => {
                for item in items {
                    let draft = self.element_draft(item, SymbolKind::Actor);
                    self.elements.push(draft);
                }
                if let Some(tail) = relation {
                    if tail.target.explicit_declaration {
                        let draft = self.element_draft(&tail.target, SymbolKind::Usecase);
                        self.elements.push(draft);
                    }
                    self.add_relationship(Self::endpoint(&items[0], true), tail);
                }
            }
            Statement::Entity { source, relation } => match relation {
                None => {
                    let mut source = source.clone();
                    source.explicit_declaration = true;
                    source.shape.get_or_insert(Shape::Ellipse);
                    let draft = self.element_draft(&source, SymbolKind::Usecase);
                    self.elements.push(draft);
                }
                Some(tail) => {
                    if source.explicit_declaration {
                        let draft = self.element_draft(source, SymbolKind::Usecase);
                        self.elements.push(draft);
                    }
                    if tail.target.explicit_declaration {
                        let draft = self.element_draft(&tail.target, SymbolKind::Usecase);
                        self.elements.push(draft);
                    }
                    self.add_relationship(
                        Self::endpoint(source, source.explicit_declaration),
                        tail,
                    );
                }
            },
            Statement::Boundary {
                name,
                metadata,
                classes,
                children,
            } => {
                self.boundaries.push(BoundaryDraft {
                    id: name.id.clone(),
                    label: name.label.clone(),
                    location: name.loc,
                    generated: name.generated,
                    classes: classes.clone(),
                });
                let previous = self.parent.replace((name.id.clone(), name.loc));
                for child in children {
                    match child {
                        BoundaryChild::Actors(items) => {
                            for item in items {
                                let draft = self.element_draft(item, SymbolKind::Actor);
                                self.elements.push(draft);
                            }
                        }
                        BoundaryChild::Entity(entity) => {
                            let mut entity = entity.clone();
                            entity.explicit_declaration = true;
                            entity.shape.get_or_insert(Shape::Ellipse);
                            let draft = self.element_draft(&entity, SymbolKind::Usecase);
                            self.elements.push(draft);
                        }
                    }
                }
                self.parent = previous;
                if let Some(m) = metadata {
                    self.metadata_assignments
                        .push((name.id.clone(), name.loc, m.clone()));
                }
            }
            Statement::MetadataAssignment {
                target,
                loc,
                metadata,
            } => self
                .metadata_assignments
                .push((target.clone(), *loc, metadata.clone())),
            Statement::Note {
                target,
                target_loc,
                label,
            } => self.notes.push(NoteDraft {
                target: target.clone(),
                target_location: *target_loc,
                label: label.clone(),
            }),
            Statement::Json {
                id,
                id_loc,
                literal,
                classes,
            } => {
                let (line, column) = self.positions.line_column(literal.0);
                let value = parse_ordered_object(&self.text[literal.0..literal.1], line, column)
                    .map_err(|e| ModelError {
                        message: e.full_message(),
                        line: e.line,
                    })?;
                self.json.push(JsonDraft {
                    id: id.clone(),
                    value,
                    location: *id_loc,
                    classes: classes.clone(),
                });
            }
            Statement::ClassDef { ids, styles } => {
                self.class_defs.push((ids.clone(), styles.clone()));
            }
            Statement::ClassAssign { targets, classes } => {
                self.class_assignments
                    .push((targets.clone(), classes.clone()));
            }
            Statement::Style {
                target,
                loc,
                styles,
            } => self
                .style_assignments
                .push((target.clone(), *loc, styles.clone())),
        }
        Ok(())
    }

    // -- finalize -----------------------------------------------------

    fn finalize(mut self) -> MResult<UsecaseDb> {
        let mut symbols: HashMap<String, SymbolOrigin> = HashMap::new();
        let mut states: IndexMap<String, ElementState> = IndexMap::new();
        let mut boundaries: IndexMap<String, BoundaryState> = IndexMap::new();
        let mut first: HashMap<String, usize> = HashMap::new();
        let record = |first: &mut HashMap<String, usize>, id: &str, offset: usize| {
            let e = first.entry(id.to_string()).or_insert(offset);
            if offset < *e {
                *e = offset;
            }
        };
        for r in &self.relationships {
            record(&mut first, &r.source.id, r.source.location.0);
            record(&mut first, &r.target.id, r.target.location.0);
        }
        for d in &self.elements {
            record(&mut first, &d.id, d.location.0);
        }
        for d in &self.boundaries {
            record(&mut first, &d.id, d.location.0);
        }
        for d in &self.json {
            record(&mut first, &d.id, d.location.0);
        }

        enum Decl {
            Element(usize),
            Boundary(usize),
            Json(usize),
            Edge(usize),
        }
        let mut declarations: Vec<(usize, Decl)> = Vec::new();
        for (i, d) in self.elements.iter().enumerate() {
            declarations.push((d.location.0, Decl::Element(i)));
        }
        for (i, d) in self.boundaries.iter().enumerate() {
            declarations.push((d.location.0, Decl::Boundary(i)));
        }
        for (i, d) in self.json.iter().enumerate() {
            declarations.push((d.location.0, Decl::Json(i)));
        }
        for (i, r) in self.relationships.iter().enumerate() {
            if let Some((_, span)) = &r.explicit_id {
                declarations.push((span.0, Decl::Edge(i)));
            }
        }
        declarations.sort_by_key(|(offset, _)| *offset);
        for (_, decl) in &declarations {
            match decl {
                Decl::Element(i) => {
                    let draft = self.elements[*i].clone();
                    self.collect_element(&draft, &mut symbols, &mut states)?;
                }
                Decl::Boundary(i) => {
                    let draft = self.boundaries[*i].clone();
                    self.collect_boundary(&draft, &mut symbols, &mut boundaries)?;
                }
                Decl::Json(i) => {
                    let (id, loc) = (self.json[*i].id.clone(), self.json[*i].location);
                    self.register_unique(&mut symbols, &id, SymbolKind::Json, loc)?;
                }
                Decl::Edge(i) => {
                    if let Some((id, loc)) = self.relationships[*i].explicit_id.clone() {
                        self.register_unique(&mut symbols, &id, SymbolKind::Edge, loc)?;
                    }
                }
            }
        }

        let by_first = |first: &HashMap<String, usize>, ids: Vec<String>| {
            let mut ids = ids;
            ids.sort_by_key(|id| first.get(id).copied().unwrap_or(0));
            ids
        };
        for id in by_first(&first, states.keys().cloned().collect()) {
            let state = states[&id].clone();
            self.set_element_model(&state);
        }
        for id in by_first(&first, boundaries.keys().cloned().collect()) {
            let s = &boundaries[&id];
            self.db.boundaries.insert(
                id.clone(),
                Boundary {
                    id: id.clone(),
                    label: s.label.text.clone(),
                    label_type: s.label.kind,
                    boundary_type: s.boundary_type.unwrap_or(BoundaryType::Rect),
                    members: Vec::new(),
                    classes: s.classes.clone(),
                    styles: Vec::new(),
                },
            );
        }
        let mut json = self.json.clone();
        json.sort_by_key(|d| first.get(&d.id).copied().unwrap_or(0));
        for d in json {
            self.db.json_nodes.insert(
                d.id.clone(),
                JsonNode {
                    id: d.id.clone(),
                    value: d.value,
                    classes: d.classes,
                    styles: Vec::new(),
                },
            );
        }

        // `materializeRelationships`.
        let mut edge_index: HashMap<String, usize> = HashMap::new();
        let mut anonymous = 0;
        let drafts = self.relationships.clone();
        for draft in &drafts {
            let source_kind =
                self.resolve_endpoint(&draft.source, &mut symbols, &mut states, &mut first)?;
            let target_kind =
                self.resolve_endpoint(&draft.target, &mut symbols, &mut states, &mut first)?;
            self.validate_relationship(draft, source_kind, target_kind, &symbols)?;
            let id = match &draft.explicit_id {
                Some((id, _)) => id.clone(),
                None => {
                    let id = format!("edge-{anonymous}");
                    anonymous += 1;
                    id
                }
            };
            edge_index.insert(id.clone(), self.db.relationships.len());
            self.db.relationships.push(Relationship {
                id,
                explicit_id: draft.explicit_id.is_some(),
                source: draft.source.id.clone(),
                target: draft.target.id.clone(),
                kind: draft.arrow.kind,
                arrow_type: draft.arrow.arrow_type,
                label: draft.arrow.label.as_ref().map(|l| (l.text.clone(), l.kind)),
                minlen: draft.arrow.minlen,
                classes: Vec::new(),
                styles: Vec::new(),
                animate: false,
                animation: None,
            });
        }

        // `reorderElements`.
        let actors = std::mem::take(&mut self.db.actors);
        let use_cases = std::mem::take(&mut self.db.use_cases);
        for id in by_first(&first, states.keys().cloned().collect()) {
            if !actors.contains_key(&id) && !use_cases.contains_key(&id) {
                let state = states[&id].clone();
                self.set_element_model(&state);
            }
            if let Some(a) = actors
                .get(&id)
                .cloned()
                .or_else(|| self.db.actors.get(&id).cloned())
            {
                self.db.actors.insert(id.clone(), a);
            } else if let Some(u) = use_cases
                .get(&id)
                .cloned()
                .or_else(|| self.db.use_cases.get(&id).cloned())
            {
                self.db.use_cases.insert(id.clone(), u);
            }
        }

        self.apply_metadata_assignments(&symbols, &mut states, &mut boundaries, &edge_index)?;
        self.validate_and_refresh(&states)?;
        self.refresh_boundaries(&mut boundaries)?;
        self.materialize_notes(&symbols)?;
        for (ids, styles) in &self.class_defs {
            for id in ids {
                self.db.class_defs.insert(id.clone(), styles.clone());
            }
        }
        self.apply_classes_and_styles(&symbols, &edge_index)?;
        if let Some(d) = self.directions.last() {
            self.db.direction = d.clone();
        }
        Ok(self.db)
    }

    fn collect_element(
        &self,
        draft: &ElementDraft,
        symbols: &mut HashMap<String, SymbolOrigin>,
        states: &mut IndexMap<String, ElementState>,
    ) -> MResult<()> {
        let draft_label = draft.generated.then_some(draft.label.text.as_str());
        if let Some(origin) = symbols.get(&draft.id) {
            let origin_label = origin
                .generated
                .then_some(origin.label.as_deref())
                .flatten();
            if origin.kind != draft.kind {
                return Err(self.conflict(
                    &format!(
                        "ID '{}' is declared as both {} and {}",
                        draft.id,
                        origin.kind.as_str(),
                        draft.kind.as_str()
                    ),
                    draft.location,
                    origin.location,
                    draft_label,
                    origin_label,
                ));
            }
            if origin.generated || draft.generated {
                return Err(self.conflict(
                    &format!(
                        "Generated ID '{}' collides with another declaration",
                        draft.id
                    ),
                    draft.location,
                    origin.location,
                    draft_label,
                    origin_label,
                ));
            }
        }
        let Some(existing) = states.get_mut(&draft.id) else {
            let mut state = ElementState {
                kind: draft.kind,
                id: draft.id.clone(),
                label: draft.label.clone(),
                location: draft.location,
                parent: draft.parent.clone(),
                shape: draft.shape,
                stereotype: draft.stereotype.clone(),
                stereotype_location: draft.stereotype.as_ref().map(|_| draft.location),
                classes: draft.classes.clone(),
                actor_type: None,
                icon: None,
                business: None,
            };
            self.apply_declaration_metadata(&mut state, draft.metadata.as_ref())?;
            states.insert(draft.id.clone(), state);
            symbols.insert(
                draft.id.clone(),
                SymbolOrigin {
                    kind: draft.kind,
                    location: draft.location,
                    generated: draft.generated,
                    label: Some(draft.label.text.clone()),
                },
            );
            return Ok(());
        };
        if existing.label.text != draft.label.text || existing.label.kind != draft.label.kind {
            return Err(self.conflict(
                &format!("ID '{}' has conflicting labels", draft.id),
                draft.location,
                existing.location,
                None,
                None,
            ));
        }
        if let (Some(a), Some(b)) = (draft.shape, existing.shape)
            && a != b
        {
            return Err(self.conflict(
                &format!("Use case '{}' has conflicting shapes", draft.id),
                draft.location,
                existing.location,
                None,
                None,
            ));
        }
        if let (Some((a, a_loc)), Some((b, b_loc))) = (&draft.parent, &existing.parent)
            && a != b
        {
            return Err(self.conflict(
                &format!(
                    "Element '{}' belongs to more than one system boundary",
                    draft.id
                ),
                *a_loc,
                *b_loc,
                None,
                None,
            ));
        }
        if let (Some(a), Some(b)) = (&draft.stereotype, &existing.stereotype)
            && a != b
        {
            return Err(self.conflict(
                &format!("Element '{}' has conflicting stereotypes", draft.id),
                draft.location,
                existing.stereotype_location.unwrap_or(existing.location),
                None,
                None,
            ));
        }
        if existing.shape.is_none() {
            existing.shape = draft.shape;
        }
        if existing.parent.is_none() {
            existing.parent = draft.parent.clone();
        }
        if existing.stereotype.is_none() {
            existing.stereotype = draft.stereotype.clone();
        }
        if existing.stereotype_location.is_none() && draft.stereotype.is_some() {
            existing.stereotype_location = Some(draft.location);
        }
        push_unique(&mut existing.classes, &draft.classes);
        let mut state = existing.clone();
        self.apply_declaration_metadata(&mut state, draft.metadata.as_ref())?;
        states.insert(draft.id.clone(), state);
        Ok(())
    }

    fn collect_boundary(
        &self,
        draft: &BoundaryDraft,
        symbols: &mut HashMap<String, SymbolOrigin>,
        states: &mut IndexMap<String, BoundaryState>,
    ) -> MResult<()> {
        let draft_label = draft.generated.then_some(draft.label.text.as_str());
        if let Some(origin) = symbols.get(&draft.id) {
            let origin_label = origin
                .generated
                .then_some(origin.label.as_deref())
                .flatten();
            if origin.kind != SymbolKind::Boundary {
                return Err(self.conflict(
                    &format!(
                        "ID '{}' is declared as both {} and boundary",
                        draft.id,
                        origin.kind.as_str()
                    ),
                    draft.location,
                    origin.location,
                    draft_label,
                    origin_label,
                ));
            }
            if origin.generated || draft.generated {
                return Err(self.conflict(
                    &format!(
                        "Generated ID '{}' collides with another declaration",
                        draft.id
                    ),
                    draft.location,
                    origin.location,
                    draft_label,
                    origin_label,
                ));
            }
        }
        if let Some(existing) = states.get_mut(&draft.id) {
            if existing.label.text != draft.label.text || existing.label.kind != draft.label.kind {
                let location = existing.location;
                return Err(self.conflict(
                    &format!("Boundary '{}' has conflicting titles", draft.id),
                    draft.location,
                    location,
                    None,
                    None,
                ));
            }
            push_unique(&mut existing.classes, &draft.classes);
            return Ok(());
        }
        states.insert(
            draft.id.clone(),
            BoundaryState {
                label: draft.label.clone(),
                location: draft.location,
                boundary_type: None,
                classes: draft.classes.clone(),
                members: Vec::new(),
            },
        );
        symbols.insert(
            draft.id.clone(),
            SymbolOrigin {
                kind: SymbolKind::Boundary,
                location: draft.location,
                generated: draft.generated,
                label: Some(draft.label.text.clone()),
            },
        );
        Ok(())
    }

    fn register_unique(
        &self,
        symbols: &mut HashMap<String, SymbolOrigin>,
        id: &str,
        kind: SymbolKind,
        location: Span,
    ) -> MResult<()> {
        if let Some(previous) = symbols.get(id) {
            let previous_label = previous
                .generated
                .then_some(previous.label.as_deref())
                .flatten();
            return Err(self.conflict(
                &format!(
                    "ID '{id}' is declared more than once ({} and {})",
                    previous.kind.as_str(),
                    kind.as_str()
                ),
                location,
                previous.location,
                None,
                previous_label,
            ));
        }
        symbols.insert(
            id.to_string(),
            SymbolOrigin {
                kind,
                location,
                generated: false,
                label: None,
            },
        );
        Ok(())
    }

    fn resolve_endpoint(
        &self,
        endpoint: &Endpoint,
        symbols: &mut HashMap<String, SymbolOrigin>,
        states: &mut IndexMap<String, ElementState>,
        first: &mut HashMap<String, usize>,
    ) -> MResult<SymbolKind> {
        if endpoint.classes_on_reference && !endpoint.declaration {
            return Err(self.fail(
                format!(
                    "Relationship endpoint '{}' uses ::: without declaring the node at {}",
                    endpoint.id,
                    self.location_text(endpoint.location)
                ),
                endpoint.location,
            ));
        }
        if let Some(origin) = symbols.get(&endpoint.id) {
            return Ok(origin.kind);
        }
        states.insert(
            endpoint.id.clone(),
            ElementState {
                kind: SymbolKind::Usecase,
                id: endpoint.id.clone(),
                label: endpoint.label.clone(),
                location: endpoint.location,
                parent: None,
                shape: Some(Shape::Ellipse),
                stereotype: None,
                stereotype_location: None,
                classes: Vec::new(),
                actor_type: None,
                icon: None,
                business: None,
            },
        );
        symbols.insert(
            endpoint.id.clone(),
            SymbolOrigin {
                kind: SymbolKind::Usecase,
                location: endpoint.location,
                generated: endpoint.generated,
                label: Some(endpoint.label.text.clone()),
            },
        );
        let e = first
            .entry(endpoint.id.clone())
            .or_insert(endpoint.location.0);
        if endpoint.location.0 < *e {
            *e = endpoint.location.0;
        }
        Ok(SymbolKind::Usecase)
    }

    fn validate_relationship(
        &self,
        draft: &RelationshipDraft,
        source: SymbolKind,
        target: SymbolKind,
        symbols: &HashMap<String, SymbolOrigin>,
    ) -> MResult<()> {
        let allowed = |k: SymbolKind| {
            matches!(
                k,
                SymbolKind::Actor | SymbolKind::Usecase | SymbolKind::Json
            )
        };
        let origin = |id: &str| symbols.get(id).map_or((0, 0), |o| o.location);
        if !allowed(source) {
            return Err(self.conflict(
                &format!(
                    "Relationship source '{}' cannot be {}",
                    draft.source.id,
                    source.as_str()
                ),
                draft.source.location,
                origin(&draft.source.id),
                None,
                None,
            ));
        }
        if !allowed(target) {
            return Err(self.conflict(
                &format!(
                    "Relationship target '{}' cannot be {}",
                    draft.target.id,
                    target.as_str()
                ),
                draft.target.location,
                origin(&draft.target.id),
                None,
                None,
            ));
        }
        let kind = draft.arrow.kind;
        if matches!(kind, RelationKind::Include | RelationKind::Extend)
            && (source != SymbolKind::Usecase || target != SymbolKind::Usecase)
        {
            return Err(self.fail(
                format!(
                    "{} relationship requires use-case endpoints at {}",
                    kind.as_str(),
                    self.location_text(draft.location)
                ),
                draft.location,
            ));
        }
        if kind == RelationKind::Generalization
            && (!matches!(source, SymbolKind::Actor | SymbolKind::Usecase) || source != target)
        {
            return Err(self.fail(
                format!(
                    "Generalization requires actor-to-actor or use-case-to-use-case endpoints at {}",
                    self.location_text(draft.location)
                ),
                draft.location,
            ));
        }
        if kind == RelationKind::Association
            && (source == SymbolKind::Json || target == SymbolKind::Json)
            && !matches!(
                draft.arrow.arrow_type,
                ArrowType::SolidArrow | ArrowType::BackArrow | ArrowType::LineSolid
            )
        {
            return Err(self.fail(
                format!(
                    "JSON relationship '{}' to '{}' permits only point, reversed-point, or markerless solid association at {}",
                    draft.source.id,
                    draft.target.id,
                    self.location_text(draft.location)
                ),
                draft.location,
            ));
        }
        Ok(())
    }

    fn apply_declaration_metadata(
        &self,
        state: &mut ElementState,
        metadata: Option<&Metadata>,
    ) -> MResult<()> {
        let Some(metadata) = metadata else {
            return Ok(());
        };
        for p in &metadata.properties {
            if state.kind == SymbolKind::Actor {
                self.apply_actor_property(state, p, false)?;
            } else if let ("business", MetaValue::Bool(b)) = (p.key.as_str(), &p.value) {
                if state.business.is_some_and(|x| x != *b) {
                    return Err(self.conflict(
                        &format!("Use case '{}' has conflicting business metadata", state.id),
                        p.loc,
                        state.location,
                        None,
                        None,
                    ));
                }
                state.business = Some(*b);
            } else {
                return Err(self.invalid_metadata(&state.id, state.kind.as_str(), p));
            }
        }
        Ok(())
    }

    fn apply_actor_property(
        &self,
        state: &mut ElementState,
        p: &MetaProp,
        replace: bool,
    ) -> MResult<()> {
        let conflict = |what: &str| {
            self.conflict(
                &format!("Actor '{}' has conflicting {what} metadata", state.id),
                p.loc,
                state.location,
                None,
                None,
            )
        };
        match (p.key.as_str(), &p.value) {
            ("type", MetaValue::Str(v))
                if matches!(v.as_str(), "normal" | "hollow" | "awesome") =>
            {
                let t = match v.as_str() {
                    "hollow" => ActorType::Hollow,
                    "awesome" => ActorType::Awesome,
                    _ => ActorType::Normal,
                };
                if !replace && state.actor_type.is_some_and(|x| x != t) {
                    return Err(conflict("type"));
                }
                state.actor_type = Some(t);
            }
            ("icon", MetaValue::Str(v)) => {
                if !replace && state.icon.as_ref().is_some_and(|x| x != v) {
                    return Err(conflict("icon"));
                }
                state.icon = Some(v.clone());
            }
            ("business", MetaValue::Bool(b)) => {
                if !replace && state.business.is_some_and(|x| x != *b) {
                    return Err(conflict("business"));
                }
                state.business = Some(*b);
            }
            _ => return Err(self.invalid_metadata(&state.id, state.kind.as_str(), p)),
        }
        Ok(())
    }

    /// `inferMetadataKind`.
    fn infer_metadata_kind(metadata: &Metadata) -> Option<&'static str> {
        let mut possible: Vec<&'static str> = vec!["actor", "usecase", "boundary", "edge"];
        for p in &metadata.properties {
            match p.key.as_str() {
                "icon" => possible = vec!["actor"],
                "animate" | "animation" => possible = vec!["edge"],
                "type" => {
                    possible = match &p.value {
                        MetaValue::Str(v) if v == "rect" || v == "package" => vec!["boundary"],
                        MetaValue::Str(v)
                            if matches!(v.as_str(), "normal" | "hollow" | "awesome") =>
                        {
                            vec!["actor"]
                        }
                        _ => Vec::new(),
                    }
                }
                "business" => possible.retain(|k| *k != "boundary" && *k != "edge"),
                _ => return None,
            }
        }
        (possible.len() == 1).then(|| possible[0])
    }

    fn apply_metadata_assignments(
        &mut self,
        symbols: &HashMap<String, SymbolOrigin>,
        states: &mut IndexMap<String, ElementState>,
        boundaries: &mut IndexMap<String, BoundaryState>,
        edges: &HashMap<String, usize>,
    ) -> MResult<()> {
        let assignments = self.metadata_assignments.clone();
        for (target, loc, metadata) in &assignments {
            let Some(origin) = symbols.get(target) else {
                let implied = Self::infer_metadata_kind(metadata)
                    .map_or(String::new(), |k| format!(" (metadata implies {k})"));
                return Err(self.fail(
                    format!(
                        "Metadata target '{target}' is unresolved{implied} at {}",
                        self.location_text(*loc)
                    ),
                    *loc,
                ));
            };
            match origin.kind {
                SymbolKind::Actor | SymbolKind::Usecase => {
                    let Some(state) = states.get_mut(target) else {
                        continue;
                    };
                    let mut s = state.clone();
                    for p in &metadata.properties {
                        if s.kind == SymbolKind::Actor {
                            self.apply_actor_property(&mut s, p, true)?;
                        } else if let ("business", MetaValue::Bool(b)) = (p.key.as_str(), &p.value)
                        {
                            s.business = Some(*b);
                        } else {
                            return Err(self.invalid_metadata(&s.id, s.kind.as_str(), p));
                        }
                    }
                    *state = s;
                }
                SymbolKind::Boundary => {
                    for p in &metadata.properties {
                        let t = match (p.key.as_str(), &p.value) {
                            ("type", MetaValue::Str(v)) if v == "rect" => BoundaryType::Rect,
                            ("type", MetaValue::Str(v)) if v == "package" => BoundaryType::Package,
                            _ => return Err(self.invalid_metadata(target, "boundary", p)),
                        };
                        if let Some(b) = boundaries.get_mut(target) {
                            b.boundary_type = Some(t);
                        }
                    }
                }
                SymbolKind::Edge => {
                    let Some(&index) = edges.get(target) else {
                        return Err(self.fail(
                            format!(
                                "Metadata target '{target}' is not an explicit edge at {}",
                                self.location_text(*loc)
                            ),
                            *loc,
                        ));
                    };
                    for p in &metadata.properties {
                        let relationship = &mut self.db.relationships[index];
                        match (p.key.as_str(), &p.value) {
                            ("animate", MetaValue::Bool(b)) => relationship.animate = *b,
                            ("animation", MetaValue::Str(v)) if v == "fast" || v == "slow" => {
                                relationship.animation = Some(v.clone());
                                relationship.animate = true;
                            }
                            _ => return Err(self.invalid_metadata(target, "edge", p)),
                        }
                    }
                }
                SymbolKind::Json => {
                    if let Some(p) = metadata.properties.first() {
                        return Err(self.invalid_metadata(target, "json", p));
                    }
                }
            }
        }
        for r in &mut self.db.relationships {
            if r.animation.is_some() {
                r.animate = true;
            }
        }
        Ok(())
    }

    fn validate_and_refresh(&mut self, states: &IndexMap<String, ElementState>) -> MResult<()> {
        for state in states.values() {
            let actor_type = if state.icon.is_some() {
                ActorType::Icon
            } else {
                state.actor_type.unwrap_or(ActorType::Normal)
            };
            if state.kind == SymbolKind::Actor {
                if state.icon.is_some()
                    && let Some(t) = state.actor_type
                    && t != ActorType::Normal
                {
                    return Err(self.fail(
                        format!(
                            "Actor '{}' cannot combine icon with type '{}' at {}",
                            state.id,
                            t.as_str(),
                            self.location_text(state.location)
                        ),
                        state.location,
                    ));
                }
                if state.business == Some(true)
                    && matches!(actor_type, ActorType::Icon | ActorType::Awesome)
                {
                    return Err(self.fail(
                        format!(
                            "Business actor '{}' must use normal or hollow geometry at {}",
                            state.id,
                            self.location_text(state.location)
                        ),
                        state.location,
                    ));
                }
            } else if state.shape.unwrap_or(Shape::Ellipse) == Shape::Rect
                && state.business == Some(true)
            {
                return Err(self.fail(
                    format!(
                        "Rectangular use case '{}' cannot be a business use case at {}",
                        state.id,
                        self.location_text(state.location)
                    ),
                    state.location,
                ));
            }
            self.set_element_model(state);
        }
        Ok(())
    }

    fn refresh_boundaries(
        &mut self,
        boundaries: &mut IndexMap<String, BoundaryState>,
    ) -> MResult<()> {
        for b in boundaries.values_mut() {
            b.members.clear();
        }
        let mut drafts = self.elements.clone();
        drafts.sort_by_key(|d| d.location.0);
        for d in &drafts {
            let Some((parent, parent_loc)) = &d.parent else {
                continue;
            };
            let Some(b) = boundaries.get_mut(parent) else {
                return Err(self.fail(
                    format!(
                        "Parent boundary '{parent}' for '{}' is unresolved at {}",
                        d.id,
                        self.location_text(*parent_loc)
                    ),
                    *parent_loc,
                ));
            };
            if !b.members.contains(&d.id) {
                b.members.push(d.id.clone());
            }
        }
        for (id, state) in boundaries.iter() {
            if let Some(model) = self.db.boundaries.get_mut(id) {
                model.boundary_type = state.boundary_type.unwrap_or(BoundaryType::Rect);
                model.members = state.members.clone();
                model.classes = state.classes.clone();
            }
        }
        Ok(())
    }

    fn materialize_notes(&mut self, symbols: &HashMap<String, SymbolOrigin>) -> MResult<()> {
        for (counter, draft) in self.notes.clone().into_iter().enumerate() {
            let Some(origin) = symbols.get(&draft.target) else {
                return Err(self.fail(
                    format!(
                        "Note target '{}' is unresolved at {}",
                        draft.target,
                        self.location_text(draft.target_location)
                    ),
                    draft.target_location,
                ));
            };
            if !matches!(origin.kind, SymbolKind::Actor | SymbolKind::Usecase) {
                return Err(self.conflict(
                    &format!(
                        "Note target '{}' must be an actor or use case, not {}",
                        draft.target,
                        origin.kind.as_str()
                    ),
                    draft.target_location,
                    origin.location,
                    None,
                    None,
                ));
            }
            let id = format!("note-{counter}");
            self.db.notes.insert(
                id.clone(),
                Note {
                    id,
                    target: draft.target.clone(),
                    label: draft.label.text.clone(),
                    label_type: draft.label.kind,
                },
            );
        }
        Ok(())
    }

    fn apply_classes_and_styles(
        &mut self,
        symbols: &HashMap<String, SymbolOrigin>,
        edges: &HashMap<String, usize>,
    ) -> MResult<()> {
        let unresolved = |b: &Builder<'_>, id: &str, loc: Span| {
            b.fail(
                format!(
                    "Class/style target '{id}' is unresolved or anonymous at {}",
                    b.location_text(loc)
                ),
                loc,
            )
        };
        for (targets, classes) in self.class_assignments.clone() {
            for (id, loc) in targets {
                let target = self.stylable(&id, symbols, edges);
                match target {
                    Some((c, _)) => push_unique(c, &classes),
                    None => return Err(unresolved(self, &id, loc)),
                }
            }
        }
        for (id, loc, styles) in self.style_assignments.clone() {
            match self.stylable(&id, symbols, edges) {
                Some((_, s)) => s.extend(styles),
                None => return Err(unresolved(self, &id, loc)),
            }
        }
        Ok(())
    }

    /// `getStylable`: the classes and styles of the element `id` names.
    fn stylable(
        &mut self,
        id: &str,
        symbols: &HashMap<String, SymbolOrigin>,
        edges: &HashMap<String, usize>,
    ) -> Option<(&mut Vec<String>, &mut Vec<String>)> {
        match symbols.get(id)?.kind {
            SymbolKind::Actor => self
                .db
                .actors
                .get_mut(id)
                .map(|a| (&mut a.classes, &mut a.styles)),
            SymbolKind::Usecase => self
                .db
                .use_cases
                .get_mut(id)
                .map(|u| (&mut u.classes, &mut u.styles)),
            SymbolKind::Boundary => self
                .db
                .boundaries
                .get_mut(id)
                .map(|b| (&mut b.classes, &mut b.styles)),
            SymbolKind::Json => self
                .db
                .json_nodes
                .get_mut(id)
                .map(|j| (&mut j.classes, &mut j.styles)),
            SymbolKind::Edge => {
                let index = *edges.get(id)?;
                let r = self.db.relationships.get_mut(index)?;
                Some((&mut r.classes, &mut r.styles))
            }
        }
    }

    /// `setElementModel(state)`.
    fn set_element_model(&mut self, state: &ElementState) {
        if state.kind == SymbolKind::Actor {
            let actor_type = if state.icon.is_some() {
                ActorType::Icon
            } else {
                state.actor_type.unwrap_or(ActorType::Normal)
            };
            self.db.use_cases.shift_remove(&state.id);
            let styles = self
                .db
                .actors
                .get(&state.id)
                .map(|a| a.styles.clone())
                .unwrap_or_default();
            self.db.actors.insert(
                state.id.clone(),
                Actor {
                    id: state.id.clone(),
                    label: state.label.text.clone(),
                    label_type: state.label.kind,
                    actor_type,
                    icon: state.icon.clone(),
                    business: state.business.unwrap_or(false),
                    stereotype: state.stereotype.clone(),
                    parent_id: state.parent.as_ref().map(|(p, _)| p.clone()),
                    classes: state.classes.clone(),
                    styles,
                },
            );
        } else {
            self.db.actors.shift_remove(&state.id);
            let styles = self
                .db
                .use_cases
                .get(&state.id)
                .map(|u| u.styles.clone())
                .unwrap_or_default();
            self.db.use_cases.insert(
                state.id.clone(),
                UseCase {
                    id: state.id.clone(),
                    label: state.label.text.clone(),
                    label_type: state.label.kind,
                    shape: state.shape.unwrap_or(Shape::Ellipse),
                    business: state.business.unwrap_or(false),
                    stereotype: state.stereotype.clone(),
                    parent_id: state.parent.as_ref().map(|(p, _)| p.clone()),
                    classes: state.classes.clone(),
                    styles,
                },
            );
        }
    }
}

/// Visits the statements and resolves the model, as
/// `usecaseVisitor.build(cst, input)` does.
pub fn build(text: &str, statements: &[Statement]) -> MResult<UsecaseDb> {
    let mut builder = Builder {
        text,
        positions: Positions::new(text),
        db: UsecaseDb::default(),
        elements: Vec::new(),
        boundaries: Vec::new(),
        json: Vec::new(),
        relationships: Vec::new(),
        notes: Vec::new(),
        metadata_assignments: Vec::new(),
        class_assignments: Vec::new(),
        style_assignments: Vec::new(),
        class_defs: Vec::new(),
        directions: Vec::new(),
        parent: None,
    };
    for statement in statements {
        builder.visit(statement)?;
    }
    builder.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::usecase::parser::parse;

    fn model(text: &str) -> MResult<UsecaseDb> {
        let text = format!("{text}\n");
        build(&text, &parse(&text).expect("parses"))
    }

    fn message(text: &str) -> String {
        model(text).expect_err("fails").message
    }

    #[test]
    fn declarations_conflict_with_both_locations() {
        assert_eq!(
            message("usecase-beta\nactor A\nactor A(\"Alice\")"),
            "ID 'A' has conflicting labels at line 3, column 7 [27,28); previous declaration at line 2, column 7 [19,20)"
        );
        assert_eq!(
            message("usecase-beta\nactor A\nA(\"x\")"),
            "ID 'A' is declared as both actor and usecase at line 3, column 1 [21,22); previous declaration at line 2, column 7 [19,20)"
        );
    }

    #[test]
    fn unresolved_metadata_names_the_implied_kind() {
        assert_eq!(
            message("usecase-beta\nX@{ animate: true }"),
            "Metadata target 'X' is unresolved (metadata implies edge) at line 2, column 1 [13,14)"
        );
    }

    #[test]
    fn json_errors_carry_the_diagram_position() {
        assert_eq!(
            message("usecase-beta\njson P@{\n  \"a\": 1,\n  \"b\": [1 2]\n}"),
            "Invalid JSON: Expected ',' or ']' after array element in JSON at position 22 (line 3 column 11) (line 4, column 11)"
        );
    }

    #[test]
    fn elements_follow_their_first_appearance() {
        let db = model(
            "usecase-beta\nCustomer --> Login\nCustomer --> Reset\nactor Customer(\"Customer\")\nLogin[Sign in]\n\"Request support\"",
        )
        .expect("builds");
        assert_eq!(
            db.use_cases.keys().collect::<Vec<_>>(),
            ["Login", "Reset", "Request_support"]
        );
        assert_eq!(db.use_cases["Login"].shape, Shape::Rect);
        assert_eq!(db.use_cases["Login"].label, "Sign in");
    }

    #[test]
    fn edge_metadata_and_styles_land_on_the_explicit_edge() {
        let db = model(
            "usecase-beta\nA e1@--> B\ne1@{ animation: slow }\nstyle e1 stroke:red\nclass e1 hot\nclassDef hot stroke-width:3px",
        )
        .expect("builds");
        let e = &db.relationships[0];
        assert_eq!(e.id, "e1");
        assert!(e.animate);
        assert_eq!(e.animation.as_deref(), Some("slow"));
        assert_eq!(e.styles, ["stroke:red"]);
        assert_eq!(e.classes, ["hot"]);
    }
}
