//! The d3-array `ticks` family and the d3-scale linear scale against values
//! node printed from the pinned oracle's `node_modules` (d3-array 3.2.4,
//! d3-scale 4.0.2, d3-format 3.1.2). `EXPECTED` is the output of a script
//! that ran each case through the real functions; the numbers are kept
//! verbatim and compared bit for bit, the labels as strings.

use kitty_markdown_helper::mermaid::layout::d3::scale_linear::{ScaleLinear, tick_format};
use kitty_markdown_helper::mermaid::layout::d3::ticks::{tick_increment, tick_step, ticks};
use serde_json::Value;

const EXPECTED: &str = r#"{"ticks":[{"a":0,"b":1,"c":10,"ticks":[0,0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8,0.9,1]},{"a":0,"b":10,"c":10,"ticks":[0,1,2,3,4,5,6,7,8,9,10]},{"a":4000,"b":11000,"c":10,"ticks":[4000,4500,5000,5500,6000,6500,7000,7500,8000,8500,9000,9500,10000,10500,11000]},{"a":-8,"b":6,"c":10,"ticks":[-8,-7,-6,-5,-4,-3,-2,-1,0,1,2,3,4,5,6]},{"a":0,"b":198.2,"c":10,"ticks":[0,20,40,60,80,100,120,140,160,180]},{"a":0,"b":600,"c":10,"ticks":[0,50,100,150,200,250,300,350,400,450,500,550,600]},{"a":1,"b":10,"c":10,"ticks":[1,2,3,4,5,6,7,8,9,10]},{"a":0,"b":30,"c":10,"ticks":[0,2,4,6,8,10,12,14,16,18,20,22,24,26,28,30]},{"a":-1,"b":10,"c":10,"ticks":[-1,0,1,2,3,4,5,6,7,8,9,10]},{"a":0.25,"b":0.5,"c":5,"ticks":[0.25,0.3,0.35,0.4,0.45,0.5]},{"a":-0.34,"b":2.4,"c":10,"ticks":[-0.2,0,0.2,0.4,0.6,0.8,1,1.2,1.4,1.6,1.8,2,2.2,2.4]},{"a":60,"b":140,"c":10,"ticks":[60,70,80,90,100,110,120,130,140]},{"a":0,"b":0.0001,"c":10,"ticks":[0,0.00001,0.00002,0.00003,0.00004,0.00005,0.00006,0.00007,0.00008,0.00009,0.0001]},{"a":1,"b":1000000,"c":3,"ticks":[500000,1000000]},{"a":-5,"b":5,"c":1,"ticks":[0]},{"a":10,"b":0,"c":10,"ticks":[10,9,8,7,6,5,4,3,2,1,0]},{"a":0,"b":1,"c":0.7,"ticks":[0]},{"a":0,"b":7,"c":1.5,"ticks":[0,5]},{"a":0,"b":0.00001,"c":10,"ticks":[0,0.000001,0.000002,0.000003,0.000004,0.000005,0.000006,0.000007,0.000008,0.000009,0.00001]},{"a":3,"b":3,"c":10,"ticks":[3]}],"step":[{"a":0,"b":1,"c":10,"step":0.1,"inc":-10},{"a":0,"b":10,"c":10,"step":1,"inc":1},{"a":4000,"b":11000,"c":10,"step":500,"inc":500},{"a":-8,"b":6,"c":10,"step":1,"inc":1},{"a":0,"b":198.2,"c":10,"step":20,"inc":20},{"a":0,"b":600,"c":10,"step":50,"inc":50},{"a":1,"b":10,"c":10,"step":1,"inc":-1},{"a":0,"b":30,"c":10,"step":2,"inc":2},{"a":-1,"b":10,"c":10,"step":1,"inc":1},{"a":0.25,"b":0.5,"c":5,"step":0.05,"inc":-20},{"a":-0.34,"b":2.4,"c":10,"step":0.2,"inc":-5},{"a":60,"b":140,"c":10,"step":10,"inc":10},{"a":0,"b":0.0001,"c":10,"step":0.00001,"inc":-100000},{"a":1,"b":1000000,"c":3,"step":500000,"inc":500000},{"a":-5,"b":5,"c":1,"step":10,"inc":10},{"a":10,"b":0,"c":10,"step":-1,"inc":null},{"a":0,"b":1,"c":0.7,"step":2,"inc":2},{"a":0,"b":7,"c":1.5,"step":5,"inc":5},{"a":0,"b":0.00001,"c":10,"step":0.000001,"inc":-1000000},{"a":3,"b":3,"c":10,"step":0,"inc":null}],"linear":[{"d":[0,10],"r":[0,100],"xs":[0,2.5,7,10,12,-1],"ys":[0,25,70,100,120,-10],"inv":[0,0.25,0.7000000000000001,1,1.2,-0.1]},{"d":[10,0],"r":[0,100],"xs":[0,5,10],"ys":[100,50,0],"inv":[10,9.5,9]},{"d":[4000,11000],"r":[487,40],"xs":[4000,7500,11000],"ys":[487,263.5,40],"inv":[-51013.42281879195,-105823.26621923938,-160633.10961968682]},{"d":[3,3],"r":[0,10],"xs":[3,4],"ys":[5,5],"inv":[2.9999999999999996,3]},{"d":[-8,6],"r":[0,300],"xs":[-8,0,6],"ys":[0,171.42857142857142,300],"inv":[-8.373333333333333,-8,-7.72]}],"format":[{"d":[0,11000],"c":null,"sp":null,"xs":[0,1000,11000,-2000],"labels":["0","1,000","11,000","−2,000"]},{"d":[0,1],"c":null,"sp":null,"xs":[0,0.1,0.3,1,-0.5],"labels":["0.0","0.1","0.3","1.0","−0.5"]},{"d":[0,1],"c":null,"sp":"","xs":[0,0.1,0.3,1],"labels":["0","0.1","0.3","1"]},{"d":[0,100],"c":null,"sp":"~f","xs":[10,100],"labels":["10","100"]},{"d":[0,0.001],"c":null,"sp":null,"xs":[0,0.0001,0.00025],"labels":["0.0000","0.0001","0.0003"]},{"d":[0,1000000],"c":null,"sp":null,"xs":[0,200000,1000000],"labels":["0","200,000","1,000,000"]},{"d":[0,1],"c":null,"sp":"e","xs":[0,0.1,0.5],"labels":["0e+0","1e-1","5e-1"]},{"d":[0,100],"c":null,"sp":"g","xs":[0,10,55],"labels":["0","1e+1","6e+1"]},{"d":[0,100],"c":null,"sp":"r","xs":[0,10,55],"labels":["0","10","60"]},{"d":[0,1],"c":null,"sp":",.3f","xs":[0.1,0.25],"labels":["0.100","0.250"]},{"d":[-1,1],"c":null,"sp":null,"xs":[-1,-0.2,0,0.2,1],"labels":["−1.0","−0.2","0.0","0.2","1.0"]}],"nice":[{"d":[0.201479,0.996679],"c":null,"nice":[0.2,1]},{"d":[0.5,0.25],"c":5,"nice":[0.5,0.25]},{"d":[-8.3,6.7],"c":null,"nice":[-10,8]},{"d":[1,99],"c":null,"nice":[0,100]},{"d":[0,0],"c":null,"nice":[0,0]},{"d":[123,456],"c":2,"nice":[0,600]}]}"#;

fn expected() -> Value {
    serde_json::from_str(EXPECTED).expect("EXPECTED is JSON")
}

fn num(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().expect("array").iter().map(num).collect()
}

fn pair(v: &Value) -> [f64; 2] {
    let n = nums(v);
    [n[0], n[1]]
}

#[test]
fn ticks_match_d3_array() {
    for case in expected()["ticks"].as_array().unwrap() {
        let (a, b, c) = (num(&case["a"]), num(&case["b"]), num(&case["c"]));
        assert_eq!(ticks(a, b, c), nums(&case["ticks"]), "ticks({a}, {b}, {c})");
    }
}

#[test]
fn tick_step_and_increment_match_d3_array() {
    for case in expected()["step"].as_array().unwrap() {
        let (a, b, c) = (num(&case["a"]), num(&case["b"]), num(&case["c"]));
        assert_eq!(
            tick_step(a, b, c),
            num(&case["step"]),
            "tickStep({a}, {b}, {c})"
        );
        if let Some(inc) = case["inc"].as_f64() {
            assert_eq!(tick_increment(a, b, c), inc, "tickIncrement({a}, {b}, {c})");
        }
    }
}

#[test]
fn linear_scale_and_invert_match_d3_scale() {
    for case in expected()["linear"].as_array().unwrap() {
        let s = ScaleLinear::new()
            .domain(pair(&case["d"]))
            .range(pair(&case["r"]));
        let xs = nums(&case["xs"]);
        let ys = nums(&case["ys"]);
        let inv = nums(&case["inv"]);
        for (i, x) in xs.iter().enumerate() {
            assert_eq!(s.scale(*x), ys[i], "scale({x}) for {}", case["d"]);
            assert_eq!(s.invert(*x), inv[i], "invert({x}) for {}", case["d"]);
        }
    }
}

#[test]
fn nice_matches_d3_scale() {
    for case in expected()["nice"].as_array().unwrap() {
        let count = case["c"].as_f64();
        let s = ScaleLinear::new().domain(pair(&case["d"])).nice(count);
        assert_eq!(
            s.domain_values(),
            pair(&case["nice"]),
            "nice for {}",
            case["d"]
        );
    }
}

#[test]
fn tick_format_matches_d3_format() {
    for case in expected()["format"].as_array().unwrap() {
        let [d0, d1] = pair(&case["d"]);
        let count = case["c"].as_f64().unwrap_or(10.0);
        let spec = case["sp"].as_str();
        let f = tick_format(d0, d1, count, spec).expect("supported specifier");
        let xs = nums(&case["xs"]);
        let labels: Vec<&str> = case["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        for (x, label) in xs.iter().zip(labels) {
            assert_eq!(
                f.format(*x),
                label,
                "format({x}) with {spec:?} for {}",
                case["d"]
            );
        }
    }
}
