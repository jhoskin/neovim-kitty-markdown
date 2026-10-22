//! Port of `themes/theme-neutral.js` (mermaid 12.2.0). Grayscale theme built
//! from `contrast`; `pie0..pie11` copy `cScale0..11` and `pie12` copies
//! `pie0`; `git0` is `darken(pie1, 25) || primaryColor`, where the `||` is
//! dead because khroma never returns an empty string.

use serde_json::{Map, Value};

use super::base::{
    cynefin, event_modeling_light, quadrant, radar, tag_label_border, wardley, xy_chart,
};
use super::default::journey_fill_types;
use super::khroma::Channel::{H, L};
use super::vars::{self, Result, Vars, adjust, darken, invert, lighten, mk_border};

const OLD_ATTRIBUTE_BACKGROUND_COLOR_ODD: &str = "#ffffff";
const OLD_ATTRIBUTE_BACKGROUND_COLOR_EVEN: &str = "#f2f2f2";

fn constructor() -> Result<Vars> {
    let mut v = Vars::new();
    let dark = v.dark_mode();
    v.set("primaryColor", "#eee");
    v.set("contrast", "#707070");
    let c = lighten(v.get("contrast"), 55.0)?;
    v.set("secondaryColor", c);
    v.set("background", "#ffffff");

    let c = adjust(v.get("primaryColor"), &[(H, -160.0)])?;
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
    let c = invert(v.get("tertiaryColor"))?;
    v.set("tertiaryTextColor", c);
    let c = invert(v.get("background"))?;
    v.set("lineColor", c);
    let c = invert(v.get("background"))?;
    v.set("textColor", c);

    v.set("mainBkg", "#eee");
    v.set("secondBkg", "calculated");
    v.set("lineColor", "#666");
    v.set("border1", "#999");
    v.set("border2", "calculated");
    v.set("note", "#ffa");
    v.set("text", "#333");
    v.set("critical", "#d42");
    v.set("done", "#bbb");
    v.set("arrowheadColor", "#333333");
    v.set("fontFamily", "\"trebuchet ms\", verdana, arial, sans-serif");
    v.set("fontSize", "16px");
    v.set("THEME_COLOR_LIMIT", 12);
    v.set("radius", 5);
    v.set("strokeWidth", 1);

    /* Flowchart variables */
    v.set("nodeBkg", "calculated");
    v.set("nodeBorder", "calculated");
    v.set("clusterBkg", "calculated");
    v.set("clusterBorder", "calculated");
    v.set("defaultLinkColor", "calculated");
    v.set("titleColor", "calculated");
    v.set("edgeLabelBackground", "white");

    /* Sequence Diagram variables */
    v.set("actorBorder", "calculated");
    v.set("actorBkg", "calculated");
    v.set("actorTextColor", "calculated");
    v.copy("actorLineColor", "actorBorder");
    v.set("signalColor", "calculated");
    v.set("signalTextColor", "calculated");
    v.set("labelBoxBkgColor", "calculated");
    v.set("labelBoxBorderColor", "calculated");
    v.set("labelTextColor", "calculated");
    v.set("loopTextColor", "calculated");
    v.set("noteBorderColor", "calculated");
    v.set("noteBkgColor", "calculated");
    v.set("noteTextColor", "calculated");
    v.set("activationBorderColor", "#666");
    v.set("activationBkgColor", "#f4f4f4");
    v.set("sequenceNumberColor", "white");

    /* Gantt chart variables */
    v.set("sectionBkgColor", "calculated");
    v.set("altSectionBkgColor", "white");
    v.set("sectionBkgColor2", "calculated");
    v.set("excludeBkgColor", "#eeeeee");
    v.set("taskBorderColor", "calculated");
    v.set("taskBkgColor", "calculated");
    v.set("taskTextLightColor", "white");
    v.set("taskTextColor", "calculated");
    v.set("taskTextDarkColor", "calculated");
    v.set("taskTextOutsideColor", "calculated");
    v.set("taskTextClickableColor", "#003163");
    v.set("activeTaskBorderColor", "calculated");
    v.set("activeTaskBkgColor", "calculated");
    v.set("gridColor", "calculated");
    v.set("doneTaskBkgColor", "calculated");
    v.set("doneTaskBorderColor", "calculated");
    v.set("critBkgColor", "calculated");
    v.set("critBorderColor", "calculated");
    v.set("todayLineColor", "calculated");
    v.set("vertLineColor", "calculated");

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

    /* ER diagram */
    v.or("rowOdd", |v| lighten(v.get("mainBkg"), 75.0))?;
    v.or_val("rowOdd", "#ffffff");
    v.or_val("rowEven", "#f4f4f4");

    /* state colors */
    v.set("labelColor", "black");

    v.set("errorBkgColor", "#552222");
    v.set("errorTextColor", "#552222");
    v.set("useGradient", true);
    v.copy("gradientStart", "primaryBorderColor");
    v.copy("gradientStop", "secondaryBorderColor");
    v.set(
        "dropShadow",
        "drop-shadow( 1px 2px 2px rgba(185,185,185,1))",
    );
    Ok(v)
}

fn update_colors(v: &mut Vars) -> Result<()> {
    let c = lighten(v.get("contrast"), 55.0)?;
    v.set("secondBkg", c);
    v.copy("border2", "contrast");

    /* Sequence Diagram variables */
    let c = lighten(v.get("border1"), 23.0)?;
    v.set("actorBorder", c);
    v.copy("actorBkg", "mainBkg");
    v.copy("actorTextColor", "text");
    v.copy("actorLineColor", "actorBorder");
    v.or_var("rectBkgColor", "tertiaryColor");
    v.copy("signalColor", "text");
    v.copy("signalTextColor", "text");
    v.copy("labelBoxBkgColor", "actorBkg");
    v.copy("labelBoxBorderColor", "actorBorder");
    v.copy("labelTextColor", "text");
    v.copy("loopTextColor", "text");
    v.set("noteBorderColor", "#999");
    v.set("noteBkgColor", "#666");
    v.set("noteTextColor", "#fff");

    /* Color Scale */
    v.or_val("cScale0", "#555");
    v.or_val("cScale1", "#F4F4F4");
    v.or_val("cScale2", "#555");
    v.or_val("cScale3", "#BBB");
    v.or_val("cScale4", "#777");
    v.or_val("cScale5", "#999");
    v.or_val("cScale6", "#DDD");
    v.or_val("cScale7", "#FFF");
    v.or_val("cScale8", "#DDD");
    v.or_val("cScale9", "#BBB");
    v.or_val("cScale10", "#999");
    v.or_val("cScale11", "#777");

    let limit = v.limit();
    for i in 0..limit {
        v.or(&format!("cScaleInv{i}"), |v| {
            invert(v.get(&format!("cScale{i}")))
        })?;
    }
    let dark = v.dark_mode();
    for i in 0..limit {
        let k = format!("cScalePeer{i}");
        if dark {
            v.or(&k, |v| lighten(v.get(&format!("cScale{i}")), 10.0))?;
        } else {
            v.or(&k, |v| darken(v.get(&format!("cScale{i}")), 10.0))?;
        }
    }

    v.or("scaleLabelColor", |v| {
        Ok(if dark {
            Value::from("black")
        } else {
            v.get("labelTextColor").clone()
        })
    })?;

    v.or_var("cScaleLabel0", "cScale1");
    v.or_var("cScaleLabel2", "cScale1");
    for i in 0..limit {
        v.or_var(&format!("cScaleLabel{i}"), "scaleLabelColor");
    }

    for i in 0..5 {
        let i_f = i as f64;
        v.or(&format!("surface{i}"), |v| {
            adjust(v.get("mainBkg"), &[(L, -(5.0 + i_f * 5.0))])
        })?;
        v.or(&format!("surfacePeer{i}"), |v| {
            adjust(v.get("mainBkg"), &[(L, -(8.0 + i_f * 5.0))])
        })?;
    }

    /* Flowchart variables */
    v.copy("nodeBkg", "mainBkg");
    v.copy("nodeBorder", "border1");
    v.copy("clusterBkg", "secondBkg");
    v.copy("clusterBorder", "border2");
    v.copy("defaultLinkColor", "lineColor");
    v.copy("titleColor", "text");

    /* Agentflow variables */
    v.or_var("flowContainerStroke", "secondaryBorderColor");

    /* Gantt chart variables */
    let c = lighten(v.get("contrast"), 30.0)?;
    v.set("sectionBkgColor", c);
    let c = lighten(v.get("contrast"), 30.0)?;
    v.set("sectionBkgColor2", c);

    let c = darken(v.get("contrast"), 10.0)?;
    v.set("taskBorderColor", c);

    v.copy("taskBkgColor", "contrast");
    v.copy("taskTextColor", "taskTextLightColor");
    v.copy("taskTextDarkColor", "text");
    v.copy("taskTextOutsideColor", "taskTextDarkColor");
    v.copy("activeTaskBorderColor", "taskBorderColor");
    v.copy("activeTaskBkgColor", "mainBkg");
    let c = lighten(v.get("border1"), 30.0)?;
    v.set("gridColor", c);

    v.copy("doneTaskBkgColor", "done");
    v.copy("doneTaskBorderColor", "lineColor");
    v.copy("critBkgColor", "critical");
    let c = darken(v.get("critBkgColor"), 10.0)?;
    v.set("critBorderColor", c);

    v.copy("todayLineColor", "critBkgColor");
    v.copy("vertLineColor", "critBkgColor");

    /* Architecture Diagram variables */
    v.copy("archEdgeColor", "lineColor");
    v.copy("archEdgeArrowColor", "lineColor");

    /* state colors */
    v.or_val("transitionColor", "#000");
    v.or_var("transitionLabelColor", "textColor");
    v.or_var("stateLabelColor", "stateBkg");
    v.or_var("stateLabelColor", "primaryTextColor");

    v.or_var("stateBkg", "mainBkg");
    v.or_var("labelBackgroundColor", "stateBkg");
    v.or_var("compositeBackground", "background");
    v.or_var("compositeBackground", "tertiaryColor");
    v.or_val("altBackground", "#f4f4f4");
    v.or_var("compositeTitleBackground", "mainBkg");
    v.or_val("stateBorder", "#000");
    v.copy("innerEndBackground", "primaryBorderColor");
    v.set("specialStateColor", "#222");

    v.or_var("errorBkgColor", "tertiaryColor");
    v.or_var("errorTextColor", "tertiaryTextColor");

    /* class */
    v.copy("classText", "primaryTextColor");

    /* journey */
    journey_fill_types(v)?;

    /* Pie diagram */
    for i in 0..limit {
        v.copy(&format!("pie{i}"), &format!("cScale{i}"));
    }
    v.copy("pie12", "pie0");
    v.or_val("pieTitleTextSize", "25px");
    v.or_var("pieTitleTextColor", "taskTextDarkColor");
    v.or_val("pieSectionTextSize", "17px");
    v.or_var("pieSectionTextColor", "textColor");
    v.or_val("pieLegendTextSize", "17px");
    v.or_var("pieLegendTextColor", "taskTextDarkColor");
    v.or_val("pieStrokeColor", "black");
    v.or_val("pieStrokeWidth", "2px");
    v.or_val("pieOuterStrokeWidth", "2px");
    v.or_val("pieOuterStrokeColor", "black");
    v.or_val("pieOpacity", "0.7");

    /* venn */
    for i in 0..8 {
        v.or_nullish(&format!("venn{}", i + 1), |v| {
            Ok(v.get(&format!("cScale{i}")).clone())
        })?;
    }
    v.or_nullish("vennTitleTextColor", |v| Ok(v.get("titleColor").clone()))?;
    v.or_nullish("vennSetTextColor", |v| Ok(v.get("textColor").clone()))?;

    /* cynefin */
    cynefin(
        v, "#8B0000", "#E8F5E9", "#E3F2FD", "#FBE9E7", "#FFF8E1", "#F3E5F5",
    );

    /* quadrant-graph */
    quadrant(v)?;

    /* xychart */
    xy_chart(
        v,
        "#EEE,#6BB8E4,#8ACB88,#C7ACD6,#E8DCC2,#FFB2A8,#FFF380,#7E8D91,#FFD8B1,#FAF3E0",
    );

    /* radar */
    radar(v);

    /* wardley */
    v.or_val("wardleyEvolutionColor", "#dc3545");
    wardley(v, "background", "background");

    /* requirement-diagram */
    v.or_var("requirementBackground", "primaryColor");
    v.or_var("requirementBorderColor", "primaryBorderColor");
    v.or_val("requirementBorderSize", "1");
    v.or_var("requirementTextColor", "primaryTextColor");
    v.or_var("relationColor", "lineColor");
    v.or_var("relationLabelBackground", "edgeLabelBackground");
    v.or_var("relationLabelColor", "actorTextColor");

    /* git */
    let c = darken(v.get("pie1"), 25.0)?;
    v.set("git0", c);
    v.or_var("git0", "primaryColor");
    let c = pie_or(v, "pie2", |v| Ok(v.get("secondaryColor").clone()))?;
    v.set("git1", c);
    let c = pie_or(v, "pie3", |v| Ok(v.get("tertiaryColor").clone()))?;
    v.set("git2", c);
    let c = pie_or(v, "pie4", |v| adjust(v.get("primaryColor"), &[(H, -30.0)]))?;
    v.set("git3", c);
    let c = pie_or(v, "pie5", |v| adjust(v.get("primaryColor"), &[(H, -60.0)]))?;
    v.set("git4", c);
    let c = pie_or(v, "pie6", |v| adjust(v.get("primaryColor"), &[(H, -90.0)]))?;
    v.set("git5", c);
    let c = pie_or(v, "pie7", |v| adjust(v.get("primaryColor"), &[(H, 60.0)]))?;
    v.set("git6", c);
    let c = pie_or(v, "pie8", |v| adjust(v.get("primaryColor"), &[(H, 120.0)]))?;
    v.set("git7", c);

    for i in 0..8 {
        v.or(&format!("gitInv{i}"), |v| invert(v.get(&format!("git{i}"))))?;
    }

    v.or_var("branchLabelColor", "labelTextColor");
    v.copy("gitBranchLabel0", "branchLabelColor");
    v.set("gitBranchLabel1", "white");
    v.copy("gitBranchLabel2", "branchLabelColor");
    v.set("gitBranchLabel3", "white");
    v.copy("gitBranchLabel4", "branchLabelColor");
    v.copy("gitBranchLabel5", "branchLabelColor");
    v.copy("gitBranchLabel6", "branchLabelColor");
    v.copy("gitBranchLabel7", "branchLabelColor");

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

/// `this.pieN || <fallback>` for the git colors.
fn pie_or(v: &Vars, pie: &str, fallback: impl FnOnce(&Vars) -> Result<Value>) -> Result<Value> {
    if v.truthy(pie) {
        Ok(v.get(pie).clone())
    } else {
        fallback(v)
    }
}

/// `getThemeVariables` of `theme-neutral.js`.
pub fn get_theme_variables(overrides: Option<&Map<String, Value>>) -> Result<Vars> {
    let mut v = constructor()?;
    vars::calculate(&mut v, overrides, update_colors)?;
    Ok(v)
}
