//! Port of `themes/theme-base.js` (mermaid 12.2.0): the `Theme` constructor,
//! `updateColors`, and the `base`-only rule in `calculate`. Statement order
//! and the `||` chains are kept as written, including the two assignments of
//! `sectionBkgColor` and `taskTextColor` and the `quadrantPointFill`
//! precedence (`a || isDark(b) ? c : d` parses as `(a || isDark(b)) ? c : d`).

use serde_json::{Map, Value};

use super::khroma::Channel::{B, G, H, L, R, S as SAT};
use super::vars::{self, Result, Vars, adjust, darken, invert, is_dark, lighten, mk_border, s};

const OLD_ATTRIBUTE_BACKGROUND_COLOR_ODD: &str = "#ffffff";
const OLD_ATTRIBUTE_BACKGROUND_COLOR_EVEN: &str = "#f2f2f2";

fn constructor() -> Vars {
    let mut v = Vars::new();
    v.set("background", "#f4f4f4");
    v.set("primaryColor", "#fff4dd");
    v.set("noteBkgColor", "#fff5ad");
    v.set("noteTextColor", "#333");
    v.set("THEME_COLOR_LIMIT", 12);
    v.set("radius", 5);
    v.set("strokeWidth", 1);
    v.set("fontFamily", "\"trebuchet ms\", verdana, arial, sans-serif");
    v.set("fontSize", "16px");
    v.set("useGradient", true);
    v.set(
        "dropShadow",
        "drop-shadow( 1px 2px 2px rgba(185,185,185,1))",
    );
    v
}

fn update_colors(v: &mut Vars) -> Result<()> {
    let dark = v.dark_mode();

    /* Main */
    v.or_val("primaryTextColor", if dark { "#eee" } else { "#333" });
    v.or("secondaryColor", |v| {
        adjust(v.get("primaryColor"), &[(H, -120.0)])
    })?;
    v.or("tertiaryColor", |v| {
        adjust(v.get("primaryColor"), &[(H, 180.0), (L, 5.0)])
    })?;

    v.or("primaryBorderColor", |v| {
        mk_border(v.get("primaryColor"), dark)
    })?;
    v.or("secondaryBorderColor", |v| {
        mk_border(v.get("secondaryColor"), dark)
    })?;
    v.or("tertiaryBorderColor", |v| {
        mk_border(v.get("tertiaryColor"), dark)
    })?;
    v.or("noteBorderColor", |v| {
        mk_border(v.get("noteBkgColor"), dark)
    })?;
    v.or_val("noteBkgColor", "#fff5ad");
    v.or_val("noteTextColor", "#333");

    v.or("secondaryTextColor", |v| invert(v.get("secondaryColor")))?;
    v.or("tertiaryTextColor", |v| invert(v.get("tertiaryColor")))?;
    v.or("lineColor", |v| invert(v.get("background")))?;
    v.or("arrowheadColor", |v| invert(v.get("background")))?;
    v.or_var("textColor", "primaryTextColor");

    v.or_var("border2", "tertiaryBorderColor");

    /* Flowchart variables */
    v.or_var("nodeBkg", "primaryColor");
    v.or_var("mainBkg", "primaryColor");
    v.or_var("nodeBorder", "primaryBorderColor");
    v.or_var("clusterBkg", "tertiaryColor");
    v.or_var("clusterBorder", "tertiaryBorderColor");
    v.or_var("defaultLinkColor", "lineColor");
    v.or_var("titleColor", "tertiaryTextColor");
    v.or("edgeLabelBackground", |v| {
        if dark {
            darken(v.get("secondaryColor"), 30.0)
        } else {
            Ok(v.get("secondaryColor").clone())
        }
    })?;

    /* Agentflow variables */
    v.or_var("flowContainerStroke", "secondaryBorderColor");
    v.or_var("nodeTextColor", "primaryTextColor");

    /* Sequence Diagram variables */
    v.or_var("actorBorder", "primaryBorderColor");
    v.or_var("actorBkg", "mainBkg");
    v.or_var("actorTextColor", "primaryTextColor");
    v.or_var("actorLineColor", "actorBorder");
    v.or_var("labelBoxBkgColor", "actorBkg");
    v.or_var("signalColor", "textColor");
    v.or_var("signalTextColor", "textColor");
    v.or_var("labelBoxBorderColor", "actorBorder");
    v.or_var("labelTextColor", "actorTextColor");
    v.or_var("loopTextColor", "actorTextColor");
    v.or("activationBorderColor", |v| {
        darken(v.get("secondaryColor"), 10.0)
    })?;
    v.or_var("activationBkgColor", "secondaryColor");
    v.or("sequenceNumberColor", |v| invert(v.get("lineColor")))?;
    v.or_var("rectBkgColor", "tertiaryColor");

    /* Gantt chart variables */
    v.or_var("sectionBkgColor", "tertiaryColor");
    v.or_val("altSectionBkgColor", "white");
    v.or_var("sectionBkgColor", "secondaryColor");
    v.or_var("sectionBkgColor2", "primaryColor");
    v.or_val("excludeBkgColor", "#eeeeee");
    v.or_var("taskBorderColor", "primaryBorderColor");
    v.or_var("taskBkgColor", "primaryColor");
    v.or_var("activeTaskBorderColor", "primaryColor");
    v.or("activeTaskBkgColor", |v| {
        lighten(v.get("primaryColor"), 23.0)
    })?;
    v.or_val("gridColor", "lightgrey");
    v.or_val("doneTaskBkgColor", "lightgrey");
    v.or_val("doneTaskBorderColor", "grey");
    v.or_val("critBorderColor", "#ff8888");
    v.or_val("critBkgColor", "red");
    v.or_val("todayLineColor", "red");
    v.or_val("vertLineColor", "navy");
    v.or_var("taskTextColor", "textColor");
    v.or_var("taskTextOutsideColor", "textColor");
    v.or_var("taskTextLightColor", "textColor");
    v.or_var("taskTextColor", "primaryTextColor");
    v.or_var("taskTextDarkColor", "textColor");
    v.or_val("taskTextClickableColor", "#003163");

    v.or_val("noteFontWeight", "normal");
    v.or_val("fontWeight", "normal");

    /* C4 */
    v.or_var("personBorder", "primaryBorderColor");
    v.or_var("personBkg", "mainBkg");

    /* ER diagram */
    if dark {
        v.or("rowOdd", |v| darken(v.get("mainBkg"), 5.0))?;
        v.or_val("rowOdd", "#ffffff");
        v.or("rowEven", |v| darken(v.get("mainBkg"), 10.0))?;
    } else {
        v.or("rowOdd", |v| lighten(v.get("mainBkg"), 75.0))?;
        v.or_val("rowOdd", "#ffffff");
        v.or("rowEven", |v| lighten(v.get("mainBkg"), 5.0))?;
    }

    /* state colors */
    v.or_var("transitionColor", "lineColor");
    v.or_var("transitionLabelColor", "textColor");
    v.or_var("stateLabelColor", "stateBkg");
    v.or_var("stateLabelColor", "primaryTextColor");

    v.or_var("stateBkg", "mainBkg");
    v.or_var("labelBackgroundColor", "stateBkg");
    v.or_var("compositeBackground", "background");
    v.or_var("compositeBackground", "tertiaryColor");
    v.or_var("altBackground", "tertiaryColor");
    v.or_var("compositeTitleBackground", "mainBkg");
    v.or_var("compositeBorder", "nodeBorder");
    v.copy("innerEndBackground", "nodeBorder");
    v.or_var("errorBkgColor", "tertiaryColor");
    v.or_var("errorTextColor", "tertiaryTextColor");
    v.or_var("transitionColor", "lineColor");
    v.copy("specialStateColor", "lineColor");

    /* Color Scale */
    v.or_var("cScale0", "primaryColor");
    v.or_var("cScale1", "secondaryColor");
    v.or_var("cScale2", "tertiaryColor");
    v.or("cScale3", |v| adjust(v.get("primaryColor"), &[(H, 30.0)]))?;
    v.or("cScale4", |v| adjust(v.get("primaryColor"), &[(H, 60.0)]))?;
    v.or("cScale5", |v| adjust(v.get("primaryColor"), &[(H, 90.0)]))?;
    v.or("cScale6", |v| adjust(v.get("primaryColor"), &[(H, 120.0)]))?;
    v.or("cScale7", |v| adjust(v.get("primaryColor"), &[(H, 150.0)]))?;
    v.or("cScale8", |v| {
        adjust(v.get("primaryColor"), &[(H, 210.0), (L, 150.0)])
    })?;
    v.or("cScale9", |v| adjust(v.get("primaryColor"), &[(H, 270.0)]))?;
    v.or("cScale10", |v| adjust(v.get("primaryColor"), &[(H, 300.0)]))?;
    v.or("cScale11", |v| adjust(v.get("primaryColor"), &[(H, 330.0)]))?;
    let limit = v.limit();
    for i in 0..limit {
        let k = format!("cScale{i}");
        let amount = if dark { 75.0 } else { 25.0 };
        let c = darken(v.get(&k), amount)?;
        v.set(&k, c);
    }

    for i in 0..limit {
        let k = format!("cScaleInv{i}");
        v.or(&k, |v| invert(v.get(&format!("cScale{i}"))))?;
    }
    for i in 0..limit {
        let k = format!("cScalePeer{i}");
        if dark {
            v.or(&k, |v| lighten(v.get(&format!("cScale{i}")), 10.0))?;
        } else {
            v.or(&k, |v| darken(v.get(&format!("cScale{i}")), 10.0))?;
        }
    }

    v.or_var("scaleLabelColor", "labelTextColor");

    for i in 0..limit {
        v.or_var(&format!("cScaleLabel{i}"), "scaleLabelColor");
    }

    let multiplier = if dark { -4.0 } else { -1.0 };
    for i in 0..5 {
        let i_f = i as f64;
        v.or(&format!("surface{i}"), |v| {
            adjust(
                v.get("mainBkg"),
                &[
                    (H, 180.0),
                    (SAT, -15.0),
                    (L, multiplier * (5.0 + i_f * 3.0)),
                ],
            )
        })?;
        v.or(&format!("surfacePeer{i}"), |v| {
            adjust(
                v.get("mainBkg"),
                &[
                    (H, 180.0),
                    (SAT, -15.0),
                    (L, multiplier * (8.0 + i_f * 3.0)),
                ],
            )
        })?;
    }

    /* class */
    v.or_var("classText", "textColor");

    /* user-journey */
    v.or_var("fillType0", "primaryColor");
    v.or_var("fillType1", "secondaryColor");
    v.or("fillType2", |v| adjust(v.get("primaryColor"), &[(H, 64.0)]))?;
    v.or("fillType3", |v| {
        adjust(v.get("secondaryColor"), &[(H, 64.0)])
    })?;
    v.or("fillType4", |v| {
        adjust(v.get("primaryColor"), &[(H, -64.0)])
    })?;
    v.or("fillType5", |v| {
        adjust(v.get("secondaryColor"), &[(H, -64.0)])
    })?;
    v.or("fillType6", |v| {
        adjust(v.get("primaryColor"), &[(H, 128.0)])
    })?;
    v.or("fillType7", |v| {
        adjust(v.get("secondaryColor"), &[(H, 128.0)])
    })?;

    /* pie */
    v.or_var("pie1", "primaryColor");
    v.or_var("pie2", "secondaryColor");
    v.or_var("pie3", "tertiaryColor");
    v.or("pie4", |v| adjust(v.get("primaryColor"), &[(L, -10.0)]))?;
    v.or("pie5", |v| adjust(v.get("secondaryColor"), &[(L, -10.0)]))?;
    v.or("pie6", |v| adjust(v.get("tertiaryColor"), &[(L, -10.0)]))?;
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
        adjust(v.get("primaryColor"), &[(H, 60.0), (L, -20.0)])
    })?;
    v.or("pie11", |v| {
        adjust(v.get("primaryColor"), &[(H, -60.0), (L, -20.0)])
    })?;
    v.or("pie12", |v| {
        adjust(v.get("primaryColor"), &[(H, 120.0), (L, -10.0)])
    })?;
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
        v, "#8B0000", "#E8F5E9", "#E3F2FD", "#FBE9E7", "#FFF8E1", "#F3E5F5",
    );

    /* radar */
    radar(v);

    /* wardley */
    v.or_val("wardleyEvolutionColor", "#dc3545");
    wardley(v, "background", "background");

    /* architecture */
    v.or_val("archEdgeColor", "#777");
    v.or_val("archEdgeArrowColor", "#777");
    v.or_val("archEdgeWidth", "3");
    v.or_val("archGroupBorderColor", "#000");
    v.or_val("archGroupBorderWidth", "2px");

    /* quadrant-graph */
    quadrant(v)?;

    /* xychart */
    xy_chart(
        v,
        "#FFF4DD,#FFD8B1,#FFA07A,#ECEFF1,#D6DBDF,#C3E0A8,#FFB6A4,#FFD74D,#738FA7,#FFFFF0",
    );

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
    v.or_var("git0", "primaryColor");
    v.or_var("git1", "secondaryColor");
    v.or_var("git2", "tertiaryColor");
    v.or("git3", |v| adjust(v.get("primaryColor"), &[(H, -30.0)]))?;
    v.or("git4", |v| adjust(v.get("primaryColor"), &[(H, -60.0)]))?;
    v.or("git5", |v| adjust(v.get("primaryColor"), &[(H, -90.0)]))?;
    v.or("git6", |v| adjust(v.get("primaryColor"), &[(H, 60.0)]))?;
    v.or("git7", |v| adjust(v.get("primaryColor"), &[(H, 120.0)]))?;
    git_shift(v, dark)?;
    for i in 0..8 {
        v.or(&format!("gitInv{i}"), |v| invert(v.get(&format!("git{i}"))))?;
    }
    v.or("branchLabelColor", |v| {
        Ok(if dark {
            s("black")
        } else {
            v.get("labelTextColor").clone()
        })
    })?;
    for i in 0..8 {
        v.or_var(&format!("gitBranchLabel{i}"), "branchLabelColor");
    }

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

    v.copy("gradientStart", "primaryBorderColor");
    v.copy("gradientStop", "secondaryBorderColor");
    Ok(())
}

/// `this.tagLabelBorder = this.tagBorder || this.primaryBorderColor`: the
/// left side reads `tagBorder`, a variable no theme ever sets, so the user
/// override of `tagLabelBorder` only wins through the second override pass.
pub(super) fn tag_label_border(v: &mut Vars) {
    let val = if v.truthy("tagBorder") {
        v.get("tagBorder").clone()
    } else {
        v.get("primaryBorderColor").clone()
    };
    v.set("tagLabelBorder", val);
}

/// The `if (this.darkMode) lighten(...) else darken(...)` block over
/// `git0..git7`, shared by base, default, and forest.
pub(super) fn git_shift(v: &mut Vars, dark: bool) -> Result<()> {
    for i in 0..8 {
        let k = format!("git{i}");
        let c = if dark {
            lighten(v.get(&k), 25.0)?
        } else {
            darken(v.get(&k), 25.0)?
        };
        v.set(&k, c);
    }
    Ok(())
}

/// The `quadrant*` block, identical in every theme. The `quadrantPointFill`
/// line keeps mermaid's precedence: `a || isDark(b) ? c : d` is
/// `(a || isDark(b)) ? lighten(b) : darken(b)`, so a user `quadrantPointFill`
/// override is thrown away here and only survives the second override pass.
pub(super) fn quadrant(v: &mut Vars) -> Result<()> {
    v.or_var("quadrant1Fill", "primaryColor");
    v.or("quadrant2Fill", |v| {
        adjust(v.get("primaryColor"), &[(R, 5.0), (G, 5.0), (B, 5.0)])
    })?;
    v.or("quadrant3Fill", |v| {
        adjust(v.get("primaryColor"), &[(R, 10.0), (G, 10.0), (B, 10.0)])
    })?;
    v.or("quadrant4Fill", |v| {
        adjust(v.get("primaryColor"), &[(R, 15.0), (G, 15.0), (B, 15.0)])
    })?;
    v.or_var("quadrant1TextFill", "primaryTextColor");
    v.or("quadrant2TextFill", |v| {
        adjust(
            v.get("primaryTextColor"),
            &[(R, -5.0), (G, -5.0), (B, -5.0)],
        )
    })?;
    v.or("quadrant3TextFill", |v| {
        adjust(
            v.get("primaryTextColor"),
            &[(R, -10.0), (G, -10.0), (B, -10.0)],
        )
    })?;
    v.or("quadrant4TextFill", |v| {
        adjust(
            v.get("primaryTextColor"),
            &[(R, -15.0), (G, -15.0), (B, -15.0)],
        )
    })?;
    let condition = v.truthy("quadrantPointFill") || is_dark(v.get("quadrant1Fill"))?;
    let point_fill = if condition {
        // khroma's `lighten(color)` with no amount is `adjustChannel(l, undefined)`:
        // `l + undefined` is NaN, `clamp` keeps NaN, and NaN !== l writes it.
        nan_lightness(v.get("quadrant1Fill"))?
    } else {
        // `darken(color)` is `adjustChannel(l, -undefined)`, also NaN.
        nan_lightness(v.get("quadrant1Fill"))?
    };
    v.set("quadrantPointFill", point_fill);
    v.or_var("quadrantPointTextFill", "primaryTextColor");
    v.or_var("quadrantXAxisTextFill", "primaryTextColor");
    v.or_var("quadrantYAxisTextFill", "primaryTextColor");
    v.or_var("quadrantInternalBorderStrokeFill", "primaryBorderColor");
    v.or_var("quadrantExternalBorderStrokeFill", "primaryBorderColor");
    v.or_var("quadrantTitleFill", "primaryTextColor");
    Ok(())
}

/// `lighten(color)` and `darken(color)` without an amount, as the quadrant
/// block calls them: the lightness becomes NaN and khroma prints
/// `hsl(h, s%, NaN%)`.
fn nan_lightness(color: &Value) -> Result<Value> {
    lighten(color, f64::NAN)
}

/// The `cynefin` object with the theme's own palette.
pub(super) fn cynefin(
    v: &mut Vars,
    cliff: &str,
    complex: &str,
    complicated: &str,
    chaotic: &str,
    clear: &str,
    confusion: &str,
) {
    let line = v.get("lineColor").clone();
    let text = v.get("textColor").clone();
    let label = v.get("primaryTextColor").clone();
    let entries = vec![
        ("domainFontSize", v.sub_or("cynefin", "domainFontSize", 16)),
        ("itemFontSize", v.sub_or("cynefin", "itemFontSize", 12)),
        (
            "boundaryColor",
            v.sub_or("cynefin", "boundaryColor", line.clone()),
        ),
        ("boundaryWidth", v.sub_or("cynefin", "boundaryWidth", 2)),
        ("cliffColor", v.sub_or("cynefin", "cliffColor", cliff)),
        ("cliffWidth", v.sub_or("cynefin", "cliffWidth", 4)),
        ("arrowColor", v.sub_or("cynefin", "arrowColor", line)),
        ("arrowWidth", v.sub_or("cynefin", "arrowWidth", 2)),
        ("complexBg", v.sub_or("cynefin", "complexBg", complex)),
        (
            "complicatedBg",
            v.sub_or("cynefin", "complicatedBg", complicated),
        ),
        ("chaoticBg", v.sub_or("cynefin", "chaoticBg", chaotic)),
        ("clearBg", v.sub_or("cynefin", "clearBg", clear)),
        ("confusionBg", v.sub_or("cynefin", "confusionBg", confusion)),
        ("textColor", v.sub_or("cynefin", "textColor", text)),
        ("labelColor", v.sub_or("cynefin", "labelColor", label)),
    ];
    v.set_object("cynefin", entries);
}

/// The `radar` object, identical in every theme.
pub(super) fn radar(v: &mut Vars) {
    let line = v.get("lineColor").clone();
    let entries = vec![
        ("axisColor", v.sub_or("radar", "axisColor", line)),
        ("axisStrokeWidth", v.sub_or("radar", "axisStrokeWidth", 2)),
        (
            "axisLabelFontSize",
            v.sub_or("radar", "axisLabelFontSize", 12),
        ),
        ("curveOpacity", v.sub_or("radar", "curveOpacity", 0.5)),
        ("curveStrokeWidth", v.sub_or("radar", "curveStrokeWidth", 2)),
        (
            "graticuleColor",
            v.sub_or("radar", "graticuleColor", "#DEDEDE"),
        ),
        (
            "graticuleStrokeWidth",
            v.sub_or("radar", "graticuleStrokeWidth", 1),
        ),
        (
            "graticuleOpacity",
            v.sub_or("radar", "graticuleOpacity", 0.3),
        ),
        ("legendBoxSize", v.sub_or("radar", "legendBoxSize", 12)),
        ("legendFontSize", v.sub_or("radar", "legendFontSize", 12)),
    ];
    v.set_object("radar", entries);
}

/// The `wardley` object. `component_fill` and `annotation_fill` name the
/// variable each theme reads for `componentFill` and `annotationFill`
/// (`background`, or `mainBkg` in the dark theme).
pub(super) fn wardley(v: &mut Vars, component_fill: &str, annotation_fill: &str) {
    let background = v.get("background").clone();
    let line = v.get("lineColor").clone();
    let text = v.get("primaryTextColor").clone();
    let grid = v.get("gridColor").clone();
    let component = v.get(component_fill).clone();
    let annotation = v.get(annotation_fill).clone();
    let evolution = v.get("wardleyEvolutionColor").clone();
    let entries = vec![
        (
            "backgroundColor",
            v.sub_or("wardley", "backgroundColor", background),
        ),
        ("axisColor", v.sub_or("wardley", "axisColor", line.clone())),
        (
            "axisTextColor",
            v.sub_or("wardley", "axisTextColor", text.clone()),
        ),
        ("gridColor", v.sub_or("wardley", "gridColor", grid)),
        (
            "componentFill",
            v.sub_or("wardley", "componentFill", component),
        ),
        (
            "componentStroke",
            v.sub_or("wardley", "componentStroke", line.clone()),
        ),
        (
            "componentLabelColor",
            v.sub_or("wardley", "componentLabelColor", text.clone()),
        ),
        (
            "linkStroke",
            v.sub_or("wardley", "linkStroke", line.clone()),
        ),
        (
            "evolutionStroke",
            v.sub_or("wardley", "evolutionStroke", evolution),
        ),
        (
            "annotationStroke",
            v.sub_or("wardley", "annotationStroke", line),
        ),
        (
            "annotationTextColor",
            v.sub_or("wardley", "annotationTextColor", text),
        ),
        (
            "annotationFill",
            v.sub_or("wardley", "annotationFill", annotation),
        ),
    ];
    v.set_object("wardley", entries);
}

/// The `xyChart` object with the theme's own `plotColorPalette`.
pub(super) fn xy_chart(v: &mut Vars, palette: &str) {
    let background = v.get("background").clone();
    let text = v.get("primaryTextColor").clone();
    let mut entries = vec![(
        "backgroundColor",
        v.sub_or("xyChart", "backgroundColor", background),
    )];
    for key in [
        "titleColor",
        "dataLabelColor",
        "legendTextColor",
        "xAxisTitleColor",
        "xAxisLabelColor",
        "xAxisTickColor",
        "xAxisLineColor",
        "yAxisTitleColor",
        "yAxisLabelColor",
        "yAxisTickColor",
        "yAxisLineColor",
    ] {
        entries.push((key, v.sub_or("xyChart", key, text.clone())));
    }
    entries.push((
        "plotColorPalette",
        v.sub_or("xyChart", "plotColorPalette", palette),
    ));
    v.set_object("xyChart", entries);
}

/// The `packet` object of the dark and forest themes; `fill` names the
/// variable read for `blockFillColor`.
pub(super) fn packet(v: &mut Vars, fill: &str) {
    let text = v.get("primaryTextColor").clone();
    let fill = v.get(fill).clone();
    let entries = vec![
        ("startByteColor", text.clone()),
        ("endByteColor", text.clone()),
        ("labelColor", text.clone()),
        ("titleColor", text.clone()),
        ("blockStrokeColor", text),
        ("blockFillColor", fill),
    ];
    v.set_object("packet", entries);
}

/// The Event Modeling block of the light themes.
pub(super) fn event_modeling_light(v: &mut Vars) {
    v.or_val("emUiFill", "white");
    v.or_val("emUiStroke", "#dbdada");
    v.or_val("emProcessorFill", "#edb3f6");
    v.or_val("emProcessorStroke", "#b88cbf");
    v.or_val("emReadModelFill", "#d3f1a2");
    v.or_val("emReadModelStroke", "#a3b732");
    v.or_val("emCommandFill", "#bcd6fe");
    v.or_val("emCommandStroke", "#679ac3");
    v.or_val("emEventFill", "#ffb778");
    v.or_val("emEventStroke", "#c19a0f");
    v.or_val("emSwimlaneBackgroundOdd", "rgb(250,250,250)");
    v.or_val("emSwimlaneBackgroundStroke", "rgb(240,240,240)");
    v.or_var("emArrowhead", "lineColor");
    v.or_var("emRelationStroke", "lineColor");
}

/// `getThemeVariables` of `theme-base.js`.
pub fn get_theme_variables(overrides: Option<&Map<String, Value>>) -> Result<Vars> {
    let mut v = constructor();
    vars::calculate(&mut v, overrides, update_colors)?;
    // `base` is the theme documented as modifiable: a `nodeBorder` override
    // turns the neo gradient off unless `useGradient` is set too.
    if let Some(o) = overrides
        && o.contains_key("nodeBorder")
        && !o.contains_key("useGradient")
    {
        v.set("useGradient", false);
    }
    Ok(v)
}
