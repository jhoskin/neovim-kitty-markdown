//! The d3-scale band scale against values node printed from the pinned
//! oracle's `node_modules` (d3-scale 4.0.2). `EXPECTED` is the script's
//! output, kept verbatim.

use kitty_markdown_helper::mermaid::layout::d3::scale_band::ScaleBand;
use serde_json::Value;

const EXPECTED: &str = r#"[{"dom":["a","b","c"],"r":[0,100],"pi":1,"po":0,"al":0.5,"values":[0,50,100],"bandwidth":0,"step":50,"unknown":true},{"dom":["a"],"r":[0,100],"pi":1,"po":0,"al":0.5,"values":[50],"bandwidth":0,"step":100,"unknown":true},{"dom":["a","b","c","d"],"r":[100,0],"pi":1,"po":0,"al":0.5,"values":[100,66.66666666666667,33.333333333333336,0],"bandwidth":0,"step":33.333333333333336,"unknown":true},{"dom":["a","b","c"],"r":[0,100],"pi":0.2,"po":0.2,"al":0.5,"values":[6.25,37.5,68.75],"bandwidth":25.000000000000004,"step":31.250000000000004,"unknown":true},{"dom":["a","b","c"],"r":[0,100],"pi":0.1,"po":0.5,"al":0,"round":true,"values":[0,25,50],"bandwidth":23,"step":25,"unknown":true},{"dom":["x","y","x"],"r":[0,10],"pi":1,"po":0,"al":0.5,"values":[0,10,0],"bandwidth":0,"step":10,"unknown":true},{"dom":["a","b"],"r":[37.5,662.5],"pi":1,"po":0,"al":0.5,"values":[37.5,662.5],"bandwidth":0,"step":625,"unknown":true}]"#;

fn num(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

#[test]
fn band_scale_matches_d3_scale() {
    let cases: Vec<Value> = serde_json::from_str(EXPECTED).expect("EXPECTED is JSON");
    for case in cases {
        let domain: Vec<String> = case["dom"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_str().unwrap().to_string())
            .collect();
        let range = [num(&case["r"][0]), num(&case["r"][1])];
        let mut s = ScaleBand::new()
            .domain(domain.clone())
            .range(range)
            .padding_inner(num(&case["pi"]))
            .padding_outer(num(&case["po"]))
            .align(num(&case["al"]));
        if case["round"].as_bool() == Some(true) {
            s = s.round(true);
        }
        let values: Vec<f64> = case["values"].as_array().unwrap().iter().map(num).collect();
        for (d, v) in domain.iter().zip(values) {
            assert_eq!(s.get(d), Some(v), "scale({d}) for {}", case["dom"]);
        }
        assert_eq!(
            s.bandwidth(),
            num(&case["bandwidth"]),
            "bandwidth for {}",
            case["dom"]
        );
        assert_eq!(s.step(), num(&case["step"]), "step for {}", case["dom"]);
        assert_eq!(s.get("zz").is_none(), case["unknown"].as_bool().unwrap());
    }
}
