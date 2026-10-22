//! Port of `themes/theme-default.js` (mermaid 12.2.0). The constructor
//! assigns `secondaryColor` twice (the `adjust` result is overwritten by
//! `#ffffde`), reassigns `background` after the `invert` calls that read it,
//! and ends with its own `updateColors()` call, so `calculate` then resets
//! the remaining `'calculated'` strings and derives a second time.

use serde_json::{Map, Value};

use super::base::{
    cynefin, event_modeling_light, git_shift, quadrant, radar, tag_label_border, wardley, xy_chart,
};
use super::khroma::Channel::{H, L};
use super::vars::{self, Result, Vars, adjust, darken, invert, lighten, mk_border, rgba};

const OLD_ATTRIBUTE_BACKGROUND_COLOR_ODD: &str = "#ffffff";
const OLD_ATTRIBUTE_BACKGROUND_COLOR_EVEN: &str = "#f2f2f2";

fn constructor() -> Result<Vars> {
    let mut v = Vars::new();
    let dark = v.dark_mode();
    v.set("background", "#f4f4f4");
    v.set("primaryColor", "#ECECFF");

    let c = adjust(v.get("primaryColor"), &[(H, 120.0)])?;
    v.set("secondaryColor", c);
    v.set("secondaryColor", "#ffffde");
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

    v.set("background", "white");
    v.set("mainBkg", "#ECECFF");
    v.set("secondBkg", "#ffffde");
    v.set("lineColor", "#333333");
    v.set("border1", "#9370DB");
    let c = mk_border(v.get("primaryColor"), dark)?;
    v.set("primaryBorderColor", c);
    v.set("border2", "#aaaa33");
    v.set("arrowheadColor", "#333333");
    v.set("fontFamily", "\"trebuchet ms\", verdana, arial, sans-serif");
    v.set("fontSize", "16px");
    v.set("labelBackground", "rgba(232,232,232, 0.8)");
    v.set("textColor", "#333");
    v.set("THEME_COLOR_LIMIT", 12);
    v.set("radius", 5);
    v.set("strokeWidth", 1);

    /* Flowchart variables */
    for k in [
        "nodeBkg",
        "nodeBorder",
        "clusterBkg",
        "clusterBorder",
        "defaultLinkColor",
        "titleColor",
        "edgeLabelBackground",
    ] {
        v.set(k, "calculated");
    }

    /* Sequence Diagram variables */
    v.set("actorBorder", "calculated");
    v.set("actorBkg", "calculated");
    v.set("actorTextColor", "black");
    v.set("actorLineColor", "calculated");
    v.set("signalColor", "calculated");
    v.set("signalTextColor", "calculated");
    v.set("labelBoxBkgColor", "calculated");
    v.set("labelBoxBorderColor", "calculated");
    v.set("labelTextColor", "calculated");
    v.set("loopTextColor", "calculated");
    v.set("noteBorderColor", "calculated");
    v.set("noteBkgColor", "#fff5ad");
    v.set("noteTextColor", "calculated");
    v.set("activationBorderColor", "#666");
    v.set("activationBkgColor", "#f4f4f4");
    v.set("sequenceNumberColor", "white");
    v.set("clusterBkg", "#FBFBFF");

    /* Gantt chart variables */
    v.set("sectionBkgColor", "calculated");
    v.set("altSectionBkgColor", "calculated");
    v.set("sectionBkgColor2", "calculated");
    v.set("excludeBkgColor", "#eeeeee");
    v.set("taskBorderColor", "calculated");
    v.set("taskBkgColor", "calculated");
    v.set("taskTextLightColor", "calculated");
    v.copy("taskTextColor", "taskTextLightColor");
    v.set("taskTextDarkColor", "calculated");
    v.copy("taskTextOutsideColor", "taskTextDarkColor");
    v.set("taskTextClickableColor", "calculated");
    v.set("activeTaskBorderColor", "calculated");
    v.set("activeTaskBkgColor", "calculated");
    v.set("gridColor", "calculated");
    v.set("doneTaskBkgColor", "calculated");
    v.set("doneTaskBorderColor", "calculated");
    v.set("critBorderColor", "calculated");
    v.set("critBkgColor", "calculated");
    v.set("todayLineColor", "calculated");
    v.set("vertLineColor", "calculated");

    v.set("sectionBkgColor", rgba(102.0, 102.0, 255.0, 0.49));
    v.set("altSectionBkgColor", "white");
    v.set("sectionBkgColor2", "#fff400");
    v.set("taskBorderColor", "#534fbc");
    v.set("taskBkgColor", "#8a90dd");
    v.set("taskTextLightColor", "white");
    v.set("taskTextColor", "calculated");
    v.set("taskTextDarkColor", "black");
    v.set("taskTextOutsideColor", "calculated");
    v.set("taskTextClickableColor", "#003163");
    v.set("activeTaskBorderColor", "#534fbc");
    v.set("activeTaskBkgColor", "#bfc7ff");
    v.set("gridColor", "lightgrey");
    v.set("doneTaskBkgColor", "lightgrey");
    v.set("doneTaskBorderColor", "grey");
    v.set("critBorderColor", "#ff8888");
    v.set("critBkgColor", "red");
    v.set("todayLineColor", "red");
    v.set("vertLineColor", "navy");

    v.or_val("noteFontWeight", "normal");
    v.or_val("fontWeight", "normal");

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
    v.set("rowOdd", "calculated");
    v.set("rowEven", "calculated");

    /* state colors */
    v.set("labelColor", "black");
    v.set("errorBkgColor", "#552222");
    v.set("errorTextColor", "#552222");

    // Neo-specific
    v.set("useGradient", false);
    v.copy("gradientStart", "primaryBorderColor");
    v.copy("gradientStop", "secondaryBorderColor");
    v.set(
        "dropShadow",
        "drop-shadow(1px 2px 2px rgba(185, 185, 185, 1))",
    );
    update_colors(&mut v)?;
    Ok(v)
}

fn update_colors(v: &mut Vars) -> Result<()> {
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

    for i in 0..5 {
        let i_f = i as f64;
        v.or(&format!("surface{i}"), |v| {
            adjust(v.get("mainBkg"), &[(H, 30.0), (L, -(5.0 + i_f * 5.0))])
        })?;
        v.or(&format!("surfacePeer{i}"), |v| {
            adjust(v.get("mainBkg"), &[(H, 30.0), (L, -(7.0 + i_f * 5.0))])
        })?;
    }
    // `this.scaleLabelColor !== 'calculated' && this.scaleLabelColor ? ... : this.labelTextColor`
    let scale_label =
        if v.get("scaleLabelColor").as_str() != Some("calculated") && v.truthy("scaleLabelColor") {
            v.get("scaleLabelColor").clone()
        } else {
            v.get("labelTextColor").clone()
        };
    v.set("scaleLabelColor", scale_label);

    if v.get("labelTextColor").as_str() != Some("calculated") {
        v.or("cScaleLabel0", |v| invert(v.get("labelTextColor")))?;
        v.or("cScaleLabel3", |v| invert(v.get("labelTextColor")))?;
        for i in 0..limit {
            v.or_var(&format!("cScaleLabel{i}"), "labelTextColor");
        }
    }

    /* Flowchart variables */
    v.copy("nodeBkg", "mainBkg");
    v.copy("nodeBorder", "border1");
    v.copy("clusterBkg", "secondBkg");
    v.copy("clusterBorder", "border2");
    v.copy("defaultLinkColor", "lineColor");
    v.copy("titleColor", "textColor");
    v.copy("edgeLabelBackground", "labelBackground");

    /* Agentflow variables */
    v.or_var("flowContainerStroke", "secondaryBorderColor");

    /* Sequence Diagram variables */
    v.copy("actorBorder", "border1");
    v.copy("actorBkg", "mainBkg");
    v.copy("labelBoxBkgColor", "actorBkg");
    v.copy("signalColor", "textColor");
    v.copy("signalTextColor", "textColor");
    v.copy("labelBoxBorderColor", "actorBorder");
    v.copy("labelTextColor", "actorTextColor");
    v.copy("loopTextColor", "actorTextColor");
    v.copy("noteBorderColor", "border2");
    v.copy("noteTextColor", "actorTextColor");
    v.copy("actorLineColor", "actorBorder");
    v.or_var("rectBkgColor", "tertiaryColor");

    /* Gantt chart variables */
    v.copy("taskTextColor", "taskTextLightColor");
    v.copy("taskTextOutsideColor", "taskTextDarkColor");

    /* Architecture Diagram variables */
    v.copy("archEdgeColor", "lineColor");
    v.copy("archEdgeArrowColor", "lineColor");

    /* Entity Relationship variables */
    v.or("rowOdd", |v| lighten(v.get("primaryColor"), 75.0))?;
    v.or_val("rowOdd", "#ffffff");
    v.or("rowEven", |v| lighten(v.get("primaryColor"), 1.0))?;

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
    v.copy("innerEndBackground", "nodeBorder");
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
    v.or("pie3", |v| adjust(v.get("tertiaryColor"), &[(L, -40.0)]))?;
    v.or("pie4", |v| adjust(v.get("primaryColor"), &[(L, -10.0)]))?;
    v.or("pie5", |v| adjust(v.get("secondaryColor"), &[(L, -30.0)]))?;
    v.or("pie6", |v| adjust(v.get("tertiaryColor"), &[(L, -20.0)]))?;
    v.or("pie7", |v| {
        adjust(v.get("primaryColor"), &[(H, 60.0), (L, -20.0)])
    })?;
    v.or("pie8", |v| {
        adjust(v.get("primaryColor"), &[(H, -60.0), (L, -40.0)])
    })?;
    v.or("pie9", |v| {
        adjust(v.get("primaryColor"), &[(H, 120.0), (L, -40.0)])
    })?;
    v.or("pie10", |v| {
        adjust(v.get("primaryColor"), &[(H, 60.0), (L, -40.0)])
    })?;
    v.or("pie11", |v| {
        adjust(v.get("primaryColor"), &[(H, -90.0), (L, -40.0)])
    })?;
    v.or("pie12", |v| {
        adjust(v.get("primaryColor"), &[(H, 120.0), (L, -30.0)])
    })?;
    pie_text(v, "taskTextDarkColor");

    /* venn */
    v.or_nullish("venn1", |v| adjust(v.get("primaryColor"), &[(L, -30.0)]))?;
    v.or_nullish("venn2", |v| adjust(v.get("secondaryColor"), &[(L, -30.0)]))?;
    v.or_nullish("venn3", |v| adjust(v.get("tertiaryColor"), &[(L, -40.0)]))?;
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
        v, "#8B0000", "#E8F5E9", "#E3F2FD", "#FBE9E7", "#FFF8E1", "#F3E5F5",
    );

    /* quadrant-graph */
    quadrant(v)?;

    /* radar */
    radar(v);

    /* wardley */
    v.or_val("wardleyEvolutionColor", "#dc3545");
    wardley(v, "background", "background");

    /* xychart */
    xy_chart(
        v,
        "#ECECFF,#8493A6,#FFC3A0,#DCDDE1,#B8E994,#D1A36F,#C3CDE6,#FFB6C1,#496078,#F8F3E3",
    );

    /* requirement-diagram */
    v.or_var("requirementBackground", "primaryColor");
    v.or_var("requirementBorderColor", "primaryBorderColor");
    v.or_val("requirementBorderSize", "1");
    v.or_var("requirementTextColor", "primaryTextColor");
    v.or_var("relationColor", "lineColor");
    v.or_var("relationLabelBackground", "labelBackground");
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
    v.or("gitInv0", |v| darken(&invert(v.get("git0"))?, 25.0))?;
    for i in 1..8 {
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

/// The unconditional `fillType0..7` block of the default, dark, forest, and
/// neutral themes.
pub(super) fn journey_fill_types(v: &mut Vars) -> Result<()> {
    v.copy("fillType0", "primaryColor");
    v.copy("fillType1", "secondaryColor");
    let c = adjust(v.get("primaryColor"), &[(H, 64.0)])?;
    v.set("fillType2", c);
    let c = adjust(v.get("secondaryColor"), &[(H, 64.0)])?;
    v.set("fillType3", c);
    let c = adjust(v.get("primaryColor"), &[(H, -64.0)])?;
    v.set("fillType4", c);
    let c = adjust(v.get("secondaryColor"), &[(H, -64.0)])?;
    v.set("fillType5", c);
    let c = adjust(v.get("primaryColor"), &[(H, 128.0)])?;
    v.set("fillType6", c);
    let c = adjust(v.get("secondaryColor"), &[(H, 128.0)])?;
    v.set("fillType7", c);
    Ok(())
}

/// The pie text variables; `title_legend` names the variable the title and
/// legend colors fall back to.
pub(super) fn pie_text(v: &mut Vars, title_legend: &str) {
    v.or_val("pieTitleTextSize", "25px");
    v.or_var("pieTitleTextColor", title_legend);
    v.or_val("pieSectionTextSize", "17px");
    v.or_var("pieSectionTextColor", "textColor");
    v.or_val("pieLegendTextSize", "17px");
    v.or_var("pieLegendTextColor", title_legend);
    v.or_val("pieStrokeColor", "black");
    v.or_val("pieStrokeWidth", "2px");
    v.or_val("pieOuterStrokeWidth", "2px");
    v.or_val("pieOuterStrokeColor", "black");
    v.or_val("pieOpacity", "0.7");
}

/// `gitBranchLabel0..7` of the default, dark, and forest themes: labels 0
/// and 3 invert `labelTextColor`, the rest copy it.
pub(super) fn git_branch_labels(v: &mut Vars) -> Result<()> {
    v.or("gitBranchLabel0", |v| invert(v.get("labelTextColor")))?;
    v.or_var("gitBranchLabel1", "labelTextColor");
    v.or_var("gitBranchLabel2", "labelTextColor");
    v.or("gitBranchLabel3", |v| invert(v.get("labelTextColor")))?;
    v.or_var("gitBranchLabel4", "labelTextColor");
    v.or_var("gitBranchLabel5", "labelTextColor");
    v.or_var("gitBranchLabel6", "labelTextColor");
    v.or_var("gitBranchLabel7", "labelTextColor");
    Ok(())
}

/// `getThemeVariables` of `theme-default.js`: the only theme whose
/// `calculate` turns the remaining `'calculated'` strings into `undefined`
/// before deriving.
pub fn get_theme_variables(overrides: Option<&Map<String, Value>>) -> Result<Vars> {
    let mut v = constructor()?;
    v.reset_calculated();
    vars::calculate(&mut v, overrides, update_colors)?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_color_is_the_literal_not_the_adjusted_hue() {
        let v = get_theme_variables(None).unwrap();
        assert_eq!(v.get("secondaryColor"), &Value::from("#ffffde"));
        assert_eq!(v.get("background"), &Value::from("white"));
        assert_eq!(v.get("lineColor"), &Value::from("#333333"));
    }
}
