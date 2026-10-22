//! Port of `themes/theme-forest.js` (mermaid 12.2.0). The constructor sets
//! `background` to `#f4f4f4` and then to `white` before `invert` reads it,
//! and derives `tertiaryTextColor` from `primaryColor`, not `tertiaryColor`.

use serde_json::{Map, Value};

use super::base::{
    cynefin, event_modeling_light, git_shift, packet, quadrant, radar, tag_label_border, wardley,
    xy_chart,
};
use super::default::{git_branch_labels, journey_fill_types, pie_text};
use super::khroma::Channel::{H, L, S as SAT};
use super::vars::{self, Result, Vars, adjust, darken, invert, lighten, mk_border, s};

const OLD_ATTRIBUTE_BACKGROUND_COLOR_ODD: &str = "#ffffff";
const OLD_ATTRIBUTE_BACKGROUND_COLOR_EVEN: &str = "#f2f2f2";

fn constructor() -> Result<Vars> {
    let mut v = Vars::new();
    let dark = v.dark_mode();
    v.set("background", "#f4f4f4");
    v.set("primaryColor", "#cde498");
    v.set("secondaryColor", "#cdffb2");
    v.set("background", "white");
    v.set("mainBkg", "#cde498");
    v.set("secondBkg", "#cdffb2");
    v.set("lineColor", "green");
    v.set("border1", "#13540c");
    v.set("border2", "#6eaa49");
    v.set("arrowheadColor", "green");
    v.set("fontFamily", "\"trebuchet ms\", verdana, arial, sans-serif");
    v.set("fontSize", "16px");

    let c = lighten(&s("#cde498"), 10.0)?;
    v.set("tertiaryColor", c);
    let c = mk_border(v.get("primaryColor"), dark)?;
    v.set("primaryBorderColor", c);
    let c = mk_border(v.get("secondaryColor"), dark)?;
    v.set("secondaryBorderColor", c);
    let c = mk_border(v.get("tertiaryColor"), dark)?;
    v.set("tertiaryBorderColor", c);
    let c = invert(v.get("primaryColor"))?;
    v.set("primaryTextColor", c);
    let c = invert(v.get("secondaryColor"))?;
    v.set("secondaryTextColor", c);
    let c = invert(v.get("primaryColor"))?;
    v.set("tertiaryTextColor", c);
    let c = invert(v.get("background"))?;
    v.set("lineColor", c);
    let c = invert(v.get("background"))?;
    v.set("textColor", c);
    v.set("THEME_COLOR_LIMIT", 12);
    v.set("radius", 5);
    v.set("strokeWidth", 1);

    /* Flowchart variables */
    v.set("nodeBkg", "calculated");
    v.set("nodeBorder", "calculated");
    v.set("clusterBkg", "calculated");
    v.set("clusterBorder", "calculated");
    v.set("defaultLinkColor", "calculated");
    v.set("titleColor", "#333");
    v.set("edgeLabelBackground", "#e8e8e8");

    /* Sequence Diagram variables */
    v.set("actorBorder", "calculated");
    v.set("actorBkg", "calculated");
    v.set("actorTextColor", "black");
    v.set("actorLineColor", "calculated");
    v.set("signalColor", "#333");
    v.set("signalTextColor", "#333");
    v.set("labelBoxBkgColor", "calculated");
    v.set("labelBoxBorderColor", "#326932");
    v.set("labelTextColor", "calculated");
    v.set("loopTextColor", "calculated");
    v.set("noteBorderColor", "calculated");
    v.set("noteBkgColor", "#fff5ad");
    v.set("noteTextColor", "calculated");
    v.set("activationBorderColor", "#666");
    v.set("activationBkgColor", "#f4f4f4");
    v.set("sequenceNumberColor", "white");

    /* Gantt chart variables */
    v.set("sectionBkgColor", "#6eaa49");
    v.set("altSectionBkgColor", "white");
    v.set("sectionBkgColor2", "#6eaa49");
    v.set("excludeBkgColor", "#eeeeee");
    v.set("taskBorderColor", "calculated");
    v.set("taskBkgColor", "#487e3a");
    v.set("taskTextLightColor", "white");
    v.set("taskTextColor", "calculated");
    v.set("taskTextDarkColor", "black");
    v.set("taskTextOutsideColor", "calculated");
    v.set("taskTextClickableColor", "#003163");
    v.set("activeTaskBorderColor", "calculated");
    v.set("activeTaskBkgColor", "calculated");
    v.set("gridColor", "lightgrey");
    v.set("doneTaskBkgColor", "lightgrey");
    v.set("doneTaskBorderColor", "grey");
    v.set("critBorderColor", "#ff8888");
    v.set("critBkgColor", "red");
    v.set("todayLineColor", "red");
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

    v.set("noteFontWeight", "normal");
    v.set("fontWeight", "normal");

    /* state colors */
    v.set("labelColor", "black");

    v.set("errorBkgColor", "#552222");
    v.set("errorTextColor", "#552222");

    v.set("useGradient", true);
    v.copy("gradientStart", "primaryBorderColor");
    v.copy("gradientStop", "secondaryBorderColor");
    v.set(
        "dropShadow",
        "drop-shadow( 1px 2px 2px rgba(185,185,185,0.5))",
    );
    Ok(v)
}

fn update_colors(v: &mut Vars) -> Result<()> {
    /* Sequence Diagram variables */
    let c = darken(v.get("mainBkg"), 20.0)?;
    v.set("actorBorder", c);
    v.copy("actorBkg", "mainBkg");
    v.copy("labelBoxBkgColor", "actorBkg");
    v.copy("labelTextColor", "actorTextColor");
    v.copy("loopTextColor", "actorTextColor");
    v.copy("noteBorderColor", "border2");
    v.copy("noteTextColor", "actorTextColor");
    v.copy("actorLineColor", "actorBorder");
    v.or_var("rectBkgColor", "tertiaryColor");

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
    v.or("cScalePeer1", |v| darken(v.get("secondaryColor"), 45.0))?;
    v.or("cScalePeer2", |v| darken(v.get("tertiaryColor"), 40.0))?;
    let limit = v.limit();
    for i in 0..limit {
        let k = format!("cScale{i}");
        let c = darken(v.get(&k), 10.0)?;
        v.set(&k, c);
        v.or(&format!("cScalePeer{i}"), |v| darken(v.get(&k), 25.0))?;
    }

    for i in 0..limit {
        v.or(&format!("cScaleInv{i}"), |v| {
            adjust(v.get(&format!("cScale{i}")), &[(H, 180.0)])
        })?;
    }

    let scale_label =
        if v.get("scaleLabelColor").as_str() != Some("calculated") && v.truthy("scaleLabelColor") {
            v.get("scaleLabelColor").clone()
        } else {
            v.get("labelTextColor").clone()
        };
    v.set("scaleLabelColor", scale_label);

    for i in 0..limit {
        v.or_var(&format!("cScaleLabel{i}"), "scaleLabelColor");
    }

    for i in 0..5 {
        let i_f = i as f64;
        v.or(&format!("surface{i}"), |v| {
            adjust(
                v.get("mainBkg"),
                &[(H, 30.0), (SAT, -30.0), (L, -(5.0 + i_f * 5.0))],
            )
        })?;
        v.or(&format!("surfacePeer{i}"), |v| {
            adjust(
                v.get("mainBkg"),
                &[(H, 30.0), (SAT, -30.0), (L, -(8.0 + i_f * 5.0))],
            )
        })?;
    }

    /* Flowchart variables */
    v.copy("nodeBkg", "mainBkg");
    v.copy("nodeBorder", "border1");
    v.copy("clusterBkg", "secondBkg");
    v.copy("clusterBorder", "border2");
    v.copy("defaultLinkColor", "lineColor");

    /* Agentflow variables */
    v.or_var("flowContainerStroke", "secondaryBorderColor");

    /* Gantt chart variables */
    v.copy("taskBorderColor", "border1");
    v.copy("taskTextColor", "taskTextLightColor");
    v.copy("taskTextOutsideColor", "taskTextDarkColor");
    v.copy("activeTaskBorderColor", "taskBorderColor");
    v.copy("activeTaskBkgColor", "mainBkg");

    /* Architecture Diagram variables */
    v.copy("archEdgeColor", "lineColor");
    v.copy("archEdgeArrowColor", "lineColor");

    /* ER diagram */
    v.or("rowOdd", |v| lighten(v.get("mainBkg"), 75.0))?;
    v.or_val("rowOdd", "#ffffff");
    v.or("rowEven", |v| lighten(v.get("mainBkg"), 20.0))?;

    /* state colors */
    v.or_var("transitionColor", "lineColor");
    v.or_var("transitionLabelColor", "textColor");
    v.or_var("stateLabelColor", "stateBkg");
    v.or_var("stateLabelColor", "primaryTextColor");

    v.or_var("stateBkg", "mainBkg");
    v.or_var("labelBackgroundColor", "stateBkg");
    v.or_var("compositeBackground", "background");
    v.or_var("compositeBackground", "tertiaryColor");
    v.or_val("altBackground", "#f0f0f0");
    v.or_var("compositeTitleBackground", "mainBkg");
    v.or_var("compositeBorder", "nodeBorder");
    v.copy("innerEndBackground", "primaryBorderColor");
    v.copy("specialStateColor", "lineColor");

    v.or_var("errorBkgColor", "tertiaryColor");
    v.or_var("errorTextColor", "tertiaryTextColor");
    v.or_var("transitionColor", "lineColor");

    /* class */
    v.copy("classText", "primaryTextColor");

    /* journey */
    journey_fill_types(v)?;

    /* pie */
    v.or_var("pie1", "primaryColor");
    v.or_var("pie2", "secondaryColor");
    v.or_var("pie3", "tertiaryColor");
    v.or("pie4", |v| adjust(v.get("primaryColor"), &[(L, -30.0)]))?;
    v.or("pie5", |v| adjust(v.get("secondaryColor"), &[(L, -30.0)]))?;
    v.or("pie6", |v| {
        adjust(v.get("tertiaryColor"), &[(H, 40.0), (L, -40.0)])
    })?;
    v.or("pie7", |v| {
        adjust(v.get("primaryColor"), &[(H, 60.0), (L, -10.0)])
    })?;
    v.or("pie8", |v| {
        adjust(v.get("primaryColor"), &[(H, -60.0), (L, -10.0)])
    })?;
    v.or("pie9", |v| {
        adjust(v.get("primaryColor"), &[(H, 120.0), (L, 0.0)])
    })?;
    v.or("pie10", |v| {
        adjust(v.get("primaryColor"), &[(H, 60.0), (L, -50.0)])
    })?;
    v.or("pie11", |v| {
        adjust(v.get("primaryColor"), &[(H, -60.0), (L, -50.0)])
    })?;
    v.or("pie12", |v| {
        adjust(v.get("primaryColor"), &[(H, 120.0), (L, -50.0)])
    })?;
    pie_text(v, "taskTextDarkColor");

    /* venn */
    v.or_nullish("venn1", |v| adjust(v.get("primaryColor"), &[(L, -30.0)]))?;
    v.or_nullish("venn2", |v| adjust(v.get("secondaryColor"), &[(L, -30.0)]))?;
    v.or_nullish("venn3", |v| adjust(v.get("tertiaryColor"), &[(L, -30.0)]))?;
    v.or_nullish("venn4", |v| {
        adjust(v.get("primaryColor"), &[(H, 60.0), (L, -30.0)])
    })?;
    v.or_nullish("venn5", |v| {
        adjust(v.get("primaryColor"), &[(H, -60.0), (L, -30.0)])
    })?;
    v.or_nullish("venn6", |v| {
        adjust(v.get("secondaryColor"), &[(H, 60.0), (L, -30.0)])
    })?;
    v.or_nullish("venn7", |v| {
        adjust(v.get("primaryColor"), &[(H, 120.0), (L, -30.0)])
    })?;
    v.or_nullish("venn8", |v| {
        adjust(v.get("secondaryColor"), &[(H, 120.0), (L, -30.0)])
    })?;
    v.or_nullish("vennTitleTextColor", |v| Ok(v.get("titleColor").clone()))?;
    v.or_nullish("vennSetTextColor", |v| Ok(v.get("textColor").clone()))?;

    /* cynefin */
    cynefin(
        v, "#8B4513", "#C8E6C9", "#DCEDC8", "#FFE0B2", "#FFF9C4", "#D7CCC8",
    );

    /* quadrant-graph */
    quadrant(v)?;

    packet(v, "mainBkg");

    /* radar */
    radar(v);

    /* wardley */
    v.or_val("wardleyEvolutionColor", "#dc3545");
    wardley(v, "background", "background");

    /* xychart */
    xy_chart(
        v,
        "#CDE498,#FF6B6B,#A0D2DB,#D7BDE2,#F0F0F0,#FFC3A0,#7FD8BE,#FF9A8B,#FAF3E0,#FFF176",
    );

    /* requirement-diagram */
    v.or_var("requirementBackground", "primaryColor");
    v.or_var("requirementBorderColor", "primaryBorderColor");
    v.or_val("requirementBorderSize", "1");
    v.or_var("requirementTextColor", "primaryTextColor");
    v.or_var("relationColor", "lineColor");
    v.or_var("relationLabelBackground", "edgeLabelBackground");
    v.or_var("relationLabelColor", "actorTextColor");

    /* git */
    v.or_var("git0", "primaryColor");
    v.or_var("git1", "secondaryColor");
    v.or_var("git2", "tertiaryColor");
    v.or("git3", |v| adjust(v.get("primaryColor"), &[(H, -30.0)]))?;
    v.or("git4", |v| adjust(v.get("primaryColor"), &[(H, -60.0)]))?;
    v.or("git5", |v| adjust(v.get("primaryColor"), &[(H, -90.0)]))?;
    v.or("git6", |v| adjust(v.get("primaryColor"), &[(H, 60.0)]))?;
    v.or("git7", |v| adjust(v.get("primaryColor"), &[(H, 120.0)]))?;
    let dark = v.dark_mode();
    git_shift(v, dark)?;
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
    event_modeling_light(v);

    /* EntityRelationship diagrams */
    v.or_val(
        "attributeBackgroundColorOdd",
        OLD_ATTRIBUTE_BACKGROUND_COLOR_ODD,
    );
    v.or_val(
        "attributeBackgroundColorEven",
        OLD_ATTRIBUTE_BACKGROUND_COLOR_EVEN,
    );
    Ok(())
}

/// `getThemeVariables` of `theme-forest.js`.
pub fn get_theme_variables(overrides: Option<&Map<String, Value>>) -> Result<Vars> {
    let mut v = constructor()?;
    vars::calculate(&mut v, overrides, update_colors)?;
    Ok(v)
}
