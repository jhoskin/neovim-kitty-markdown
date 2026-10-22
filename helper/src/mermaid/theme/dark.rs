//! Port of `themes/theme-dark.js` (mermaid 12.2.0). `calculate` here has no
//! `'calculated'` reset, so `labelColor` stays the literal string
//! `'calculated'` in the output; `cScale12` is set although the color
//! loops stop at `THEME_COLOR_LIMIT`; `pie0..pie11` copy `cScale0..11`
//! and the base `pie12` never appears.

use serde_json::{Map, Value};

use super::base::{cynefin, packet, quadrant, radar, tag_label_border, wardley, xy_chart};
use super::default::{git_branch_labels, journey_fill_types, pie_text};
use super::khroma::Channel::{H, L, S as SAT};
use super::vars::{self, Result, Vars, adjust, darken, invert, lighten, mk_border, rgba, s};

fn constructor() -> Result<Vars> {
    let mut v = Vars::new();
    let dark = v.dark_mode();
    v.set("background", "#333");
    v.set("primaryColor", "#1f2020");
    let c = lighten(v.get("primaryColor"), 16.0)?;
    v.set("secondaryColor", c);
    let c = adjust(v.get("primaryColor"), &[(H, -160.0)])?;
    v.set("tertiaryColor", c);
    let c = invert(v.get("background"))?;
    v.set("primaryBorderColor", c);
    let c = mk_border(v.get("secondaryColor"), dark)?;
    v.set("secondaryBorderColor", c);
    let c = mk_border(v.get("tertiaryColor"), dark)?;
    v.set("tertiaryBorderColor", c);
    let c = invert(v.get("primaryColor"))?;
    v.set("primaryTextColor", c);
    let c = invert(v.get("secondaryColor"))?;
    v.set("secondaryTextColor", c);
    let c = invert(v.get("tertiaryColor"))?;
    v.set("tertiaryTextColor", c);
    let c = invert(v.get("background"))?;
    v.set("lineColor", c);
    let c = invert(v.get("background"))?;
    v.set("textColor", c);

    v.set("mainBkg", "#1f2020");
    v.set("secondBkg", "calculated");
    v.set("mainContrastColor", "lightgrey");
    let c = lighten(&invert(&s("#323D47"))?, 10.0)?;
    v.set("darkTextColor", c);
    v.set("lineColor", "calculated");
    v.set("border1", "#ccc");
    v.set("border2", rgba(255.0, 255.0, 255.0, 0.25));
    v.set("arrowheadColor", "calculated");
    v.set("fontFamily", "\"trebuchet ms\", verdana, arial, sans-serif");
    v.set("fontSize", "16px");
    v.set("labelBackground", "#181818");
    v.set("textColor", "#ccc");
    v.set("THEME_COLOR_LIMIT", 12);
    v.set("radius", 5);
    v.set("strokeWidth", 1);

    /* Flowchart variables */
    v.set("nodeBkg", "calculated");
    v.set("nodeBorder", "calculated");
    v.set("clusterBkg", "calculated");
    v.set("clusterBorder", "calculated");
    v.set("defaultLinkColor", "calculated");
    v.set("titleColor", "#F9FFFE");
    v.set("edgeLabelBackground", "calculated");

    /* Sequence Diagram variables */
    for k in [
        "actorBorder",
        "actorBkg",
        "actorTextColor",
        "actorLineColor",
        "signalColor",
        "signalTextColor",
        "labelBoxBkgColor",
        "labelBoxBorderColor",
        "labelTextColor",
        "loopTextColor",
        "noteBorderColor",
    ] {
        v.set(k, "calculated");
    }
    v.set("noteBkgColor", "#fff5ad");
    v.set("noteTextColor", "calculated");
    v.set("activationBorderColor", "calculated");
    v.set("activationBkgColor", "calculated");
    v.set("sequenceNumberColor", "black");
    v.set("clusterBkg", "#302F3D");

    /* Gantt chart variables */
    let c = darken(&s("#EAE8D9"), 30.0)?;
    v.set("sectionBkgColor", c);
    v.set("altSectionBkgColor", "calculated");
    v.set("sectionBkgColor2", "#EAE8D9");
    let c = darken(v.get("sectionBkgColor"), 10.0)?;
    v.set("excludeBkgColor", c);
    v.set("taskBorderColor", rgba(255.0, 255.0, 255.0, 70.0));
    v.set("taskBkgColor", "calculated");
    v.set("taskTextColor", "calculated");
    v.set("taskTextLightColor", "calculated");
    v.set("taskTextOutsideColor", "calculated");
    v.set("taskTextClickableColor", "#003163");
    v.set("activeTaskBorderColor", rgba(255.0, 255.0, 255.0, 50.0));
    v.set("activeTaskBkgColor", "#81B1DB");
    v.set("gridColor", "calculated");
    v.set("doneTaskBkgColor", "calculated");
    v.set("doneTaskBorderColor", "grey");
    v.set("critBorderColor", "#E83737");
    v.set("critBkgColor", "#E83737");
    v.set("taskTextDarkColor", "calculated");
    v.set("todayLineColor", "#DB5757");
    v.set("vertLineColor", "#00BFFF");

    /* C4 Context Diagram variables */
    v.copy("personBorder", "primaryBorderColor");
    v.copy("personBkg", "mainBkg");

    /* Architecture Diagram variables */
    v.set("archEdgeColor", "calculated");
    v.set("archEdgeArrowColor", "calculated");
    v.set("archEdgeWidth", "3");
    v.copy("archGroupBorderColor", "primaryBorderColor");
    v.set("archGroupBorderWidth", "2px");

    /* Entity Relationship variables */
    v.or("rowOdd", |v| lighten(v.get("mainBkg"), 5.0))?;
    v.or_val("rowOdd", "#ffffff");
    v.or("rowEven", |v| darken(v.get("mainBkg"), 10.0))?;

    /* state colors */
    v.set("labelColor", "calculated");

    v.set("errorBkgColor", "#a44141");
    v.set("errorTextColor", "#ddd");
    v.set("useGradient", true);
    v.copy("gradientStart", "primaryBorderColor");
    v.copy("gradientStop", "secondaryBorderColor");
    v.set(
        "dropShadow",
        "drop-shadow( 1px 2px 2px rgba(185,185,185,1))",
    );

    v.or_val("noteFontWeight", "normal");
    v.or_val("fontWeight", "normal");
    Ok(v)
}

fn update_colors(v: &mut Vars) -> Result<()> {
    let c = lighten(v.get("mainBkg"), 16.0)?;
    v.set("secondBkg", c);
    v.copy("lineColor", "mainContrastColor");
    v.copy("arrowheadColor", "mainContrastColor");

    /* Flowchart variables */
    v.copy("nodeBkg", "mainBkg");
    v.copy("nodeBorder", "border1");
    v.copy("clusterBkg", "secondBkg");
    v.copy("clusterBorder", "border2");
    v.copy("defaultLinkColor", "lineColor");
    let c = lighten(v.get("labelBackground"), 25.0)?;
    v.set("edgeLabelBackground", c);

    /* Agentflow variables */
    v.or_var("flowContainerStroke", "secondaryBorderColor");

    /* Sequence Diagram variables */
    v.copy("actorBorder", "border1");
    v.copy("actorBkg", "mainBkg");
    v.copy("actorTextColor", "mainContrastColor");
    v.copy("actorLineColor", "actorBorder");
    v.copy("signalColor", "mainContrastColor");
    v.copy("signalTextColor", "mainContrastColor");
    v.copy("labelBoxBkgColor", "actorBkg");
    v.copy("labelBoxBorderColor", "actorBorder");
    v.copy("labelTextColor", "mainContrastColor");
    v.copy("loopTextColor", "mainContrastColor");
    v.copy("noteBorderColor", "secondaryBorderColor");
    v.copy("noteBkgColor", "secondBkg");
    v.copy("noteTextColor", "secondaryTextColor");
    v.copy("activationBorderColor", "border1");
    v.copy("activationBkgColor", "secondBkg");
    v.or_var("rectBkgColor", "tertiaryColor");

    /* Gantt chart variables */
    v.copy("altSectionBkgColor", "background");
    let c = lighten(v.get("mainBkg"), 23.0)?;
    v.set("taskBkgColor", c);
    v.copy("taskTextColor", "darkTextColor");
    v.copy("taskTextLightColor", "mainContrastColor");
    v.copy("taskTextOutsideColor", "taskTextLightColor");
    v.copy("gridColor", "mainContrastColor");
    v.copy("doneTaskBkgColor", "mainContrastColor");
    let c = invert(v.get("doneTaskBkgColor"))?;
    v.set("taskTextDarkColor", c);

    /* Architecture Diagram variables */
    v.copy("archEdgeColor", "lineColor");
    v.copy("archEdgeArrowColor", "lineColor");

    /* state colors */
    v.or_var("transitionColor", "lineColor");
    v.or_var("transitionLabelColor", "textColor");
    v.or_var("stateLabelColor", "stateBkg");
    v.or_var("stateLabelColor", "primaryTextColor");
    v.or_var("stateBkg", "mainBkg");
    v.or_var("labelBackgroundColor", "stateBkg");
    v.or_var("compositeBackground", "background");
    v.or_var("compositeBackground", "tertiaryColor");
    v.or_val("altBackground", "#555");
    v.or_var("compositeTitleBackground", "mainBkg");
    v.or_var("compositeBorder", "nodeBorder");
    v.copy("innerEndBackground", "primaryBorderColor");
    v.set("specialStateColor", "#f4f4f4");

    v.or_var("errorBkgColor", "tertiaryColor");
    v.or_var("errorTextColor", "tertiaryTextColor");

    journey_fill_types(v)?;

    /* cScale */
    v.or_val("cScale1", "#0b0000");
    v.or_val("cScale2", "#4d1037");
    v.or_val("cScale3", "#3f5258");
    v.or_val("cScale4", "#4f2f1b");
    v.or_val("cScale5", "#6e0a0a");
    v.or_val("cScale6", "#3b0048");
    v.or_val("cScale7", "#995a01");
    v.or_val("cScale8", "#154706");
    v.or_val("cScale9", "#161722");
    v.or_val("cScale10", "#00296f");
    v.or_val("cScale11", "#01629c");
    v.or_val("cScale12", "#010029");

    /* Color Scale */
    v.or_var("cScale0", "primaryColor");
    v.or_var("cScale1", "secondaryColor");
    v.or_var("cScale2", "tertiaryColor");
    v.or("cScale3", |v| adjust(v.get("primaryColor"), &[(H, 30.0)]))?;
    v.or("cScale4", |v| adjust(v.get("primaryColor"), &[(H, 60.0)]))?;
    v.or("cScale5", |v| adjust(v.get("primaryColor"), &[(H, 90.0)]))?;
    v.or("cScale6", |v| adjust(v.get("primaryColor"), &[(H, 120.0)]))?;
    v.or("cScale7", |v| adjust(v.get("primaryColor"), &[(H, 150.0)]))?;
    v.or("cScale8", |v| adjust(v.get("primaryColor"), &[(H, 210.0)]))?;
    v.or("cScale9", |v| adjust(v.get("primaryColor"), &[(H, 270.0)]))?;
    v.or("cScale10", |v| adjust(v.get("primaryColor"), &[(H, 300.0)]))?;
    v.or("cScale11", |v| adjust(v.get("primaryColor"), &[(H, 330.0)]))?;

    let limit = v.limit();
    for i in 0..limit {
        v.or(&format!("cScaleInv{i}"), |v| {
            invert(v.get(&format!("cScale{i}")))
        })?;
    }
    for i in 0..limit {
        v.or(&format!("cScalePeer{i}"), |v| {
            lighten(v.get(&format!("cScale{i}")), 10.0)
        })?;
    }

    for i in 0..5 {
        let i_f = i as f64;
        v.or(&format!("surface{i}"), |v| {
            adjust(
                v.get("mainBkg"),
                &[(H, 30.0), (SAT, -30.0), (L, -(-10.0 + i_f * 4.0))],
            )
        })?;
        v.or(&format!("surfacePeer{i}"), |v| {
            adjust(
                v.get("mainBkg"),
                &[(H, 30.0), (SAT, -30.0), (L, -(-7.0 + i_f * 4.0))],
            )
        })?;
    }

    let dark = v.dark_mode();
    v.or("scaleLabelColor", |v| {
        Ok(if dark {
            s("black")
        } else {
            v.get("labelTextColor").clone()
        })
    })?;

    for i in 0..limit {
        v.or_var(&format!("cScaleLabel{i}"), "scaleLabelColor");
    }

    /* Pie diagram */
    for i in 0..limit {
        v.copy(&format!("pie{i}"), &format!("cScale{i}"));
    }
    pie_text(v, "mainContrastColor");

    /* venn */
    for i in 0..8 {
        v.or_nullish(&format!("venn{}", i + 1), |v| {
            lighten(v.get(&format!("cScale{i}")), 30.0)
        })?;
    }
    v.or_nullish("vennTitleTextColor", |v| Ok(v.get("titleColor").clone()))?;
    v.or_nullish("vennSetTextColor", |v| Ok(v.get("textColor").clone()))?;

    /* cynefin */
    cynefin(
        v, "#FF6B6B", "#1B5E20", "#0D47A1", "#BF360C", "#F57F17", "#4A148C",
    );

    /* quadrant-graph */
    quadrant(v)?;

    /* xychart */
    xy_chart(
        v,
        "#3498db,#2ecc71,#e74c3c,#f1c40f,#bdc3c7,#ffffff,#34495e,#9b59b6,#1abc9c,#e67e22",
    );

    packet(v, "background");

    /* radar */
    radar(v);

    /* wardley */
    v.or_val("wardleyEvolutionColor", "#ff6b6b");
    wardley(v, "mainBkg", "mainBkg");

    /* class */
    v.copy("classText", "primaryTextColor");

    /* requirement-diagram */
    v.or_var("requirementBackground", "primaryColor");
    v.or_var("requirementBorderColor", "primaryBorderColor");
    v.or_val("requirementBorderSize", "1");
    v.or_var("requirementTextColor", "primaryTextColor");
    v.or_var("relationColor", "lineColor");
    v.or("relationLabelBackground", |v| {
        if dark {
            darken(v.get("secondaryColor"), 30.0)
        } else {
            Ok(v.get("secondaryColor").clone())
        }
    })?;
    v.or_var("relationLabelColor", "actorTextColor");

    /* git */
    let c = lighten(v.get("secondaryColor"), 20.0)?;
    v.set("git0", c);
    let c = lighten(
        &pie_or(v, "pie2", |v| Ok(v.get("secondaryColor").clone()))?,
        20.0,
    )?;
    v.set("git1", c);
    let c = lighten(
        &pie_or(v, "pie3", |v| Ok(v.get("tertiaryColor").clone()))?,
        20.0,
    )?;
    v.set("git2", c);
    let c = lighten(
        &pie_or(v, "pie4", |v| adjust(v.get("primaryColor"), &[(H, -30.0)]))?,
        20.0,
    )?;
    v.set("git3", c);
    let c = lighten(
        &pie_or(v, "pie5", |v| adjust(v.get("primaryColor"), &[(H, -60.0)]))?,
        20.0,
    )?;
    v.set("git4", c);
    let c = lighten(
        &pie_or(v, "pie6", |v| adjust(v.get("primaryColor"), &[(H, -90.0)]))?,
        10.0,
    )?;
    v.set("git5", c);
    let c = lighten(
        &pie_or(v, "pie7", |v| adjust(v.get("primaryColor"), &[(H, 60.0)]))?,
        10.0,
    )?;
    v.set("git6", c);
    let c = lighten(
        &pie_or(v, "pie8", |v| adjust(v.get("primaryColor"), &[(H, 120.0)]))?,
        20.0,
    )?;
    v.set("git7", c);
    for i in 0..8 {
        v.or(&format!("gitInv{i}"), |v| invert(v.get(&format!("git{i}"))))?;
    }
    git_branch_labels(v)?;

    v.or_var("tagLabelColor", "primaryTextColor");
    v.or_var("tagLabelBackground", "primaryColor");
    tag_label_border(v);
    v.or_val("tagLabelFontSize", "10px");
    v.or_var("commitLabelColor", "secondaryTextColor");
    v.or_var("commitLabelBackground", "secondaryColor");
    v.or_val("commitLabelFontSize", "10px");

    /* Event Modeling diagrams */
    v.or_val("emUiFill", "#2d2d2d");
    v.or_val("emUiStroke", "#555");
    v.or("emProcessorFill", |_| lighten(&s("#5a3d5c"), 10.0))?;
    v.or_val("emProcessorStroke", "#8a6d8c");
    v.or("emReadModelFill", |_| lighten(&s("#3d5a2d"), 10.0))?;
    v.or_val("emReadModelStroke", "#6d8c5c");
    v.or("emCommandFill", |_| lighten(&s("#2d3d5a"), 10.0))?;
    v.or_val("emCommandStroke", "#5c6d8c");
    v.or("emEventFill", |_| lighten(&s("#5a452d"), 10.0))?;
    v.or_val("emEventStroke", "#8c755c");
    v.or("emSwimlaneBackgroundOdd", |v| {
        lighten(v.get("background"), 5.0)
    })?;
    v.or("emSwimlaneBackgroundStroke", |v| {
        lighten(v.get("background"), 12.0)
    })?;
    v.or_var("emArrowhead", "lineColor");
    v.or_var("emRelationStroke", "lineColor");

    /* EntityRelationship diagrams */
    v.or("attributeBackgroundColorOdd", |v| {
        lighten(v.get("background"), 12.0)
    })?;
    v.or("attributeBackgroundColorEven", |v| {
        lighten(v.get("background"), 2.0)
    })?;

    v.or_val("nodeBorder", "#999");
    Ok(())
}

/// `this.pieN || <fallback>`: the git colors read the pie variables, which
/// the pie loop above has already filled from `cScale`.
fn pie_or(v: &Vars, pie: &str, fallback: impl FnOnce(&Vars) -> Result<Value>) -> Result<Value> {
    if v.truthy(pie) {
        Ok(v.get(pie).clone())
    } else {
        fallback(v)
    }
}

/// `getThemeVariables` of `theme-dark.js`.
pub fn get_theme_variables(overrides: Option<&Map<String, Value>>) -> Result<Vars> {
    let mut v = constructor()?;
    vars::calculate(&mut v, overrides, update_colors)?;
    Ok(v)
}
