//! `wardleyDb.ts`, `wardleyBuilder.ts` (`WardleyBuilder`), and the
//! `populateDb` step of `wardleyParser.ts`.
//!
//! Coordinates are percentages from here on: `x` is the evolution and `y`
//! the visibility, both 0 to 100. The source writes `[visibility,
//! evolution]` and accepts 0 to 1 or 0 to 100.

use indexmap::IndexMap;

use super::parser::{Ast, LabelOffset};
use crate::mermaid::theme::khroma::js_number;

/// `WardleyNode`. `x` and `y` are always set: every `addNode` call in
/// `populateDb` passes numbers, so `build` never throws.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub class_name: Option<String>,
    pub label_offset_x: Option<f64>,
    pub label_offset_y: Option<f64>,
    pub in_pipeline: bool,
    pub is_pipeline_parent: bool,
    pub inertia: bool,
    pub source_strategy: Option<Strategy>,
}

/// The `(build)`, `(buy)`, `(outsource)`, and `(market)` decorators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Build,
    Buy,
    Outsource,
    Market,
}

impl Strategy {
    fn parse(image: &str) -> Option<Strategy> {
        match image {
            "build" => Some(Strategy::Build),
            "buy" => Some(Strategy::Buy),
            "outsource" => Some(Strategy::Outsource),
            "market" => Some(Strategy::Market),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Forward,
    Backward,
    Bidirectional,
}

/// `WardleyLink`.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub dashed: bool,
    pub label: Option<String>,
    pub flow: Option<Flow>,
}

/// `WardleyTrend`: an `evolve` arrow to `target_x`.
#[derive(Debug, Clone, PartialEq)]
pub struct Trend {
    pub node_id: String,
    pub target_x: f64,
    pub target_y: f64,
}

/// `WardleyPipeline`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub node_id: String,
    pub component_ids: Vec<String>,
}

/// `WardleyAnnotation`. `populateDb` gives every annotation one
/// coordinate.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub number: f64,
    pub coordinates: Vec<(f64, f64)>,
    pub text: String,
}

/// A `WardleyNote`, `WardleyAccelerator`, or `WardleyDeaccelerator`.
#[derive(Debug, Clone, PartialEq)]
pub struct Marked {
    pub text: String,
    pub x: f64,
    pub y: f64,
}

/// `WardleyAxesConfig`. The grammar sets only the stages; `xLabel` and
/// `yLabel` keep the renderer's defaults.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Axes {
    pub stages: Vec<String>,
    pub stage_boundaries: Vec<f64>,
}

/// An `Error` that `populateDb` throws, with the source line of the
/// statement that caused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbError {
    pub message: String,
    pub line: u32,
}

/// The builder state behind `getWardleyData`, plus the common db title.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WardleyDb {
    title: String,
    /// A JS `Map`: insertion order, and a key set again keeps its place.
    pub nodes: IndexMap<String, Node>,
    pub links: Vec<Link>,
    pub trends: IndexMap<String, Trend>,
    pub pipelines: IndexMap<String, Pipeline>,
    pub annotations: Vec<Annotation>,
    pub notes: Vec<Marked>,
    pub accelerators: Vec<Marked>,
    pub deaccelerators: Vec<Marked>,
    pub annotations_box: Option<(f64, f64)>,
    pub axes: Axes,
    pub size: Option<(f64, f64)>,
}

/// `toPercent`: a value up to 1 is a fraction, a larger one a percentage.
fn to_percent(value: f64, context: &str, line: u32) -> Result<f64, DbError> {
    let normalized = if value <= 1.0 { value * 100.0 } else { value };
    if !(0.0..=100.0).contains(&normalized) {
        return Err(DbError {
            message: format!(
                "{context} must be between 0-1 (decimal) or 0-100 (percentage). Received: {}",
                js_number(value)
            ),
            line,
        });
    }
    Ok(normalized)
}

/// `toCoordinates(visibility, evolution, context)`: the evolution is
/// checked first, as the object literal evaluates `x` first.
fn to_coordinates(
    visibility: f64,
    evolution: f64,
    context: &str,
    line: u32,
) -> Result<(f64, f64), DbError> {
    let x = to_percent(evolution, &format!("{context} evolution"), line)?;
    let y = to_percent(visibility, &format!("{context} visibility"), line)?;
    Ok((x, y))
}

/// `getFlowFromPort`.
fn flow_from_port(port: Option<&str>) -> Option<Flow> {
    match port? {
        "+<>" => Some(Flow::Bidirectional),
        "+<" => Some(Flow::Backward),
        "+>" => Some(Flow::Forward),
        _ => None,
    }
}

/// `extractFlowFromArrow`: a `+'label'...` arrow carries a flow and a
/// label.
fn flow_from_arrow(arrow: Option<&str>) -> (Option<Flow>, Option<String>) {
    let Some(arrow) = arrow.filter(|a| a.starts_with('+')) else {
        return (None, None);
    };
    // `/^\+'([^']*)'/`.
    let label = arrow
        .strip_prefix("+'")
        .and_then(|rest| rest.find('\'').map(|end| rest[..end].to_string()));
    let flow = if arrow.contains("<>") {
        Some(Flow::Bidirectional)
    } else if arrow.contains('<') {
        Some(Flow::Backward)
    } else if arrow.contains('>') {
        Some(Flow::Forward)
    } else {
        None
    };
    (flow, label)
}

/// The arguments of `db.addNode` after the node id.
struct NodeArgs<'a> {
    label: &'a str,
    x: f64,
    y: f64,
    class_name: &'a str,
    offset: Option<LabelOffset>,
    inertia: bool,
    strategy: Option<Strategy>,
}

impl WardleyDb {
    pub fn new() -> WardleyDb {
        WardleyDb::default()
    }

    /// `setDiagramTitle`: `sanitizeText` leaves plain text as it is;
    /// markup in a title is out of scope here.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    /// `WardleyBuilder.addNode`: the new values win, except that an unset
    /// class name or label offset keeps the old one, and the pipeline
    /// flags stay.
    fn add_node(&mut self, id: &str, args: NodeArgs<'_>) {
        let existing = self.nodes.get(id);
        let node = Node {
            id: id.to_string(),
            label: args.label.to_string(),
            x: args.x,
            y: args.y,
            class_name: Some(args.class_name.to_string()),
            label_offset_x: args
                .offset
                .map(|o| o.x)
                .or_else(|| existing.and_then(|n| n.label_offset_x)),
            label_offset_y: args
                .offset
                .map(|o| o.y)
                .or_else(|| existing.and_then(|n| n.label_offset_y)),
            in_pipeline: existing.is_some_and(|n| n.in_pipeline),
            is_pipeline_parent: existing.is_some_and(|n| n.is_pipeline_parent),
            inertia: args.inertia,
            source_strategy: args.strategy,
        };
        self.nodes.insert(id.to_string(), node);
    }

    /// `WardleyBuilder.resolveNodeId`: the id itself, else the first node
    /// with that label (a pipeline component has the id
    /// `<parent>_<name>`), else the name unchanged.
    fn resolve_node_id(&self, name: &str) -> String {
        if self.nodes.contains_key(name) {
            return name.to_string();
        }
        self.nodes
            .iter()
            .find(|(_, n)| n.label == name)
            .map_or_else(|| name.to_string(), |(id, _)| id.clone())
    }

    /// `populateDb`.
    pub fn populate(&mut self, ast: &Ast) -> Result<(), DbError> {
        // `populateCommonDb`: an empty title is falsy and leaves the front
        // matter title.
        if let Some(title) = ast.title.as_deref().filter(|t| !t.is_empty()) {
            self.title = title.to_string();
        }
        if let Some(size) = ast.size {
            self.size = Some((size.width, size.height));
        }
        if let Some(stages) = &ast.evolution {
            self.axes.stages = stages
                .iter()
                .map(|s| match &s.second_name {
                    Some(second) => {
                        format!(
                            "{} / {}",
                            super::parser::js_trim(&s.name),
                            super::parser::js_trim(second)
                        )
                    }
                    None => super::parser::js_trim(&s.name).to_string(),
                })
                .collect();
            self.axes.stage_boundaries = stages.iter().filter_map(|s| s.boundary).collect();
        }
        for anchor in &ast.anchors {
            let context = format!("Anchor \"{}\"", anchor.name);
            let (x, y) = to_coordinates(anchor.first, anchor.second, &context, anchor.line)?;
            self.add_node(
                &anchor.name,
                NodeArgs {
                    label: &anchor.name,
                    x,
                    y,
                    class_name: "anchor",
                    offset: None,
                    inertia: false,
                    strategy: None,
                },
            );
        }
        for component in &ast.components {
            let context = format!("Component \"{}\"", component.name);
            let (x, y) = to_coordinates(
                component.visibility,
                component.evolution,
                &context,
                component.line,
            )?;
            self.add_node(
                &component.name,
                NodeArgs {
                    label: &component.name,
                    x,
                    y,
                    class_name: "component",
                    offset: component.label,
                    inertia: component.inertia,
                    strategy: component.strategy.as_deref().and_then(Strategy::parse),
                },
            );
        }
        for note in &ast.notes {
            let context = format!("Note \"{}\"", note.name);
            let (x, y) = to_coordinates(note.first, note.second, &context, note.line)?;
            self.notes.push(Marked {
                text: note.name.clone(),
                x,
                y,
            });
        }
        for pipeline in &ast.pipelines {
            let Some(parent_y) = self.nodes.get(&pipeline.parent).map(|n| n.y) else {
                return Err(DbError {
                    message: format!(
                        "Pipeline \"{}\" must reference an existing component with coordinates.",
                        pipeline.parent
                    ),
                    line: pipeline.line,
                });
            };
            // `startPipeline`: a second pipeline for the same parent starts
            // over in the first one's place.
            self.pipelines.insert(
                pipeline.parent.clone(),
                Pipeline {
                    node_id: pipeline.parent.clone(),
                    component_ids: Vec::new(),
                },
            );
            if let Some(node) = self.nodes.get_mut(&pipeline.parent) {
                node.is_pipeline_parent = true;
            }
            for component in &pipeline.components {
                let id = format!("{}_{}", pipeline.parent, component.name);
                let context = format!("Pipeline component \"{}\" evolution", component.name);
                let x = to_percent(component.evolution, &context, component.line)?;
                self.add_node(
                    &id,
                    NodeArgs {
                        label: &component.name,
                        x,
                        y: parent_y,
                        class_name: "pipeline-component",
                        offset: component.label,
                        inertia: false,
                        strategy: None,
                    },
                );
                // `addPipelineComponent`.
                if let Some(p) = self.pipelines.get_mut(&pipeline.parent) {
                    p.component_ids.push(id.clone());
                }
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.in_pipeline = true;
                }
            }
        }
        for link in &ast.links {
            let dashed = link
                .arrow
                .as_deref()
                .is_some_and(|a| a.contains("-.->") || a.contains(".-."));
            let mut flow = flow_from_port(link.from_port.as_deref())
                .or_else(|| flow_from_port(link.to_port.as_deref()));
            let (arrow_flow, flow_label) = flow_from_arrow(link.arrow.as_deref());
            if flow.is_none() {
                flow = arrow_flow;
            }
            let label = flow_label.or_else(|| link.link_label.clone());
            self.links.push(Link {
                source: self.resolve_node_id(&link.from),
                target: self.resolve_node_id(&link.to),
                dashed,
                label,
                flow,
            });
        }
        for evolve in &ast.evolves {
            if let Some(node_y) = self.nodes.get(&evolve.component).map(|n| n.y) {
                let context = format!("Evolve target for \"{}\"", evolve.component);
                let target_x = to_percent(evolve.target, &context, evolve.line)?;
                self.trends.insert(
                    evolve.component.clone(),
                    Trend {
                        node_id: evolve.component.clone(),
                        target_x,
                        target_y: node_y,
                    },
                );
            }
        }
        if let Some(first) = ast.annotations.first() {
            self.annotations_box = Some(to_coordinates(
                first.x,
                first.y,
                "Annotations box",
                first.line,
            )?);
        }
        for annotation in &ast.annotation {
            let context = format!("Annotation {}", js_number(annotation.number));
            let (x, y) = to_coordinates(annotation.x, annotation.y, &context, annotation.line)?;
            self.annotations.push(Annotation {
                number: annotation.number,
                coordinates: vec![(x, y)],
                text: annotation.text.clone(),
            });
        }
        for (list, source, kind) in [
            (&mut self.accelerators, &ast.accelerators, "Accelerator"),
            (
                &mut self.deaccelerators,
                &ast.deaccelerators,
                "Deaccelerator",
            ),
        ] {
            for item in source {
                let context = format!("{kind} \"{}\"", item.name);
                let (x, y) = to_coordinates(item.first, item.second, &context, item.line)?;
                list.push(Marked {
                    text: item.name.clone(),
                    x,
                    y,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::wardley::parser::parse;

    fn db(text: &str) -> Result<WardleyDb, DbError> {
        let ast = parse(text).unwrap_or_else(|e| panic!("{text:?}: {}", e.message));
        let mut db = WardleyDb::new();
        db.populate(&ast)?;
        Ok(db)
    }

    #[test]
    fn coordinates_swap_and_scale() {
        let db = db("wardley-beta\nanchor A [0.95, 0.63]\ncomponent B [0.5, 50.5] label [-5, 3] (buy) inertia\n")
            .unwrap();
        let a = &db.nodes["A"];
        assert_eq!((a.x, a.y), (63.0, 95.0));
        assert_eq!(a.class_name.as_deref(), Some("anchor"));
        let b = &db.nodes["B"];
        assert_eq!((b.x, b.y), (50.5, 50.0));
        assert_eq!(
            (b.label_offset_x, b.label_offset_y),
            (Some(-5.0), Some(3.0))
        );
        assert_eq!(b.source_strategy, Some(Strategy::Buy));
        assert!(b.inertia);
    }

    #[test]
    fn links_resolve_flows_labels_and_pipeline_names() {
        let db = db(concat!(
            "wardley-beta\ncomponent P [0.4, 0.6]\npipeline P {\n component \"S Q\" [0.5]\n}\n",
            "P -.-> S Q\nP +'lbl'<> P; ann\nP -> X; note text \nP +< X\n",
        ))
        .unwrap();
        assert_eq!(db.nodes["P_S Q"].y, 40.0);
        assert!(db.nodes["P_S Q"].in_pipeline);
        assert!(db.nodes["P"].is_pipeline_parent);
        let l = &db.links;
        assert_eq!(l[0].target, "P_S Q");
        assert!(l[0].dashed);
        assert_eq!(
            (l[1].flow, l[1].label.as_deref()),
            (Some(Flow::Bidirectional), Some("lbl"))
        );
        assert_eq!(
            (l[2].flow, l[2].label.as_deref()),
            (None, Some("note text"))
        );
        assert_eq!(l[3].flow, Some(Flow::Backward));
    }

    #[test]
    fn stages_and_boundaries() {
        let db = db("wardley-beta\nevolution A@0.3 / B -> C -> D@1.0\n").unwrap();
        assert_eq!(db.axes.stages, vec!["A / B", "C", "D"]);
        assert_eq!(db.axes.stage_boundaries, vec![0.3, 1.0]);
    }

    #[test]
    fn errors_name_the_statement() {
        let err = db("wardley-beta\n\ncomponent A [0.5, 150.5]\n").unwrap_err();
        assert_eq!(
            err,
            DbError {
                message: "Component \"A\" evolution must be between 0-1 (decimal) or 0-100 (percentage). Received: 150.5".to_string(),
                line: 3,
            }
        );
        let err = db("wardley-beta\npipeline A {\n component B [0.5]\n}\n").unwrap_err();
        assert_eq!(
            err.message,
            "Pipeline \"A\" must reference an existing component with coordinates."
        );
        // An `evolve` of an unknown component is dropped before the range
        // check.
        assert!(
            db("wardley-beta\nevolve Nope 150.5\n")
                .unwrap()
                .trends
                .is_empty()
        );
    }
}
