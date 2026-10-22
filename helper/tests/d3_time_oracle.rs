//! The d3 ports against values printed by the pinned d3 in
//! `helper/oracle/node_modules` (d3-time 3.1.0, d3-time-format 4.1.0,
//! d3-scale 4.0.2, d3-axis 3.0.0). `EXPECT` is the output of a node script
//! run with `TZ=America/Los_Angeles`; the tests pin the same zone before the
//! first time zone lookup, because every value is a local time.
//!
//! The script (kept out of the repo) built each probe with
//! `new Date(y, m, d, ...)` and printed `floor`, `ceil`, `offset`, `count`,
//! `range`, `every(...).range`, `scaleTime().ticks()`, the default
//! `tickFormat`, `nice`, `rangeRound` mappings, `timeFormat` for a set of
//! specifiers, and the axis tick translates.

use std::sync::Once;

use kitty_markdown_helper::mermaid::layout::d3::axis::{Axis, Orient};
use kitty_markdown_helper::mermaid::layout::d3::scale_time::ScaleTime;
use kitty_markdown_helper::mermaid::layout::d3::time::{self, Interval, Ms};
use kitty_markdown_helper::mermaid::layout::d3::time_format;
use serde_json::Value;

const EXPECT: &str = r#"{
 "intervals": [
  {
   "name": "day",
   "input": 1394397907003,
   "floor": 1394352000000,
   "ceil": 1394434800000,
   "offset3": 1394657107003,
   "offsetm2": 1394228707003,
   "count_from_2014": 67
  },
  {
   "name": "sunday",
   "input": 1389801600000,
   "floor": 1389513600000,
   "ceil": 1390118400000,
   "offset3": 1391616000000,
   "offsetm2": 1388592000000,
   "count_from_2014": 2
  },
  {
   "name": "monday",
   "input": 1389801600000,
   "floor": 1389600000000,
   "ceil": 1390204800000,
   "offset3": 1391616000000,
   "offsetm2": 1388592000000,
   "count_from_2014": 2
  },
  {
   "name": "month",
   "input": 1396278000000,
   "floor": 1393660800000,
   "ceil": 1396335600000,
   "offset3": 1404226800000,
   "offsetm2": 1391184000000,
   "count_from_2014": 2
  },
  {
   "name": "year",
   "input": 1402815600000,
   "floor": 1388563200000,
   "ceil": 1420099200000,
   "offset3": 1497510000000,
   "offsetm2": 1339743600000,
   "count_from_2014": 0
  },
  {
   "name": "hour",
   "input": 1394357400000,
   "floor": 1394355600000,
   "ceil": 1394359200000,
   "offset3": 1394368200000,
   "offsetm2": 1394350200000,
   "count_from_2014": 1609
  },
  {
   "name": "minute",
   "input": 1414917020000,
   "floor": 1414917000000,
   "ceil": 1414917060000,
   "offset3": 1414917200000,
   "offsetm2": 1414916900000,
   "count_from_2014": 439230
  }
 ],
 "ranges": {
  "days_march_dst": [
   1394179200000,
   1394265600000,
   1394352000000,
   1394434800000,
   1394521200000
  ],
  "weeks_monday": [
   1678089600000,
   1678690800000,
   1679295600000,
   1679900400000
  ],
  "every2days": [
   1390982400000,
   1391155200000,
   1391241600000,
   1391414400000,
   1391587200000
  ],
  "every2weeks_sunday": [
   1389513600000,
   1390723200000,
   1391932800000,
   1393142400000
  ],
  "every3months": [
   1364799600000,
   1372662000000,
   1380610800000,
   1388563200000,
   1396335600000,
   1404198000000,
   1412146800000
  ],
  "every5years": [
   1104566400000,
   1262332800000,
   1420099200000,
   1577865600000,
   1735718400000
  ],
  "every15min": [
   1388628000000,
   1388628900000,
   1388629800000
  ],
  "every500ms": [
   1388563200500,
   1388563201000,
   1388563201500,
   1388563202000
  ]
 },
 "ticks": [
  {
   "name": "classic",
   "domain": [
    1388563200000,
    1392883200000
   ],
   "ticks": [
    1388908800000,
    1389513600000,
    1390118400000,
    1390723200000,
    1391328000000,
    1391932800000,
    1392537600000
   ],
   "labels": [
    "Jan 05",
    "Jan 12",
    "Jan 19",
    "Jan 26",
    "Feb 02",
    "Feb 09",
    "Feb 16"
   ],
   "nice": [
    1388304000000,
    1393142400000
   ]
  },
  {
   "name": "one_day",
   "domain": [
    1388563200000,
    1388649600000
   ],
   "ticks": [
    1388563200000,
    1388574000000,
    1388584800000,
    1388595600000,
    1388606400000,
    1388617200000,
    1388628000000,
    1388638800000,
    1388649600000
   ],
   "labels": [
    "2014",
    "03 AM",
    "06 AM",
    "09 AM",
    "12 PM",
    "03 PM",
    "06 PM",
    "09 PM",
    "Thu 02"
   ],
   "nice": [
    1388563200000,
    1388649600000
   ]
  },
  {
   "name": "hours",
   "domain": [
    1388627340000,
    1388628720000
   ],
   "ticks": [
    1388627400000,
    1388627700000,
    1388628000000,
    1388628300000,
    1388628600000
   ],
   "labels": [
    "05:50",
    "05:55",
    "06 PM",
    "06:05",
    "06:10"
   ],
   "nice": [
    1388627100000,
    1388628900000
   ]
  },
  {
   "name": "years",
   "domain": [
    -2208960000000,
    -1104508800000
   ],
   "ticks": [
    -2208960000000,
    -2051193600000,
    -1893427200000,
    -1735660800000,
    -1577894400000,
    -1420041600000,
    -1262275200000,
    -1104508800000
   ],
   "labels": [
    "1900",
    "1905",
    "1910",
    "1915",
    "1920",
    "1925",
    "1930",
    "1935"
   ],
   "nice": [
    -2208960000000,
    -1104508800000
   ]
  },
  {
   "name": "decade",
   "domain": [
    978336000000,
    1717225200000
   ],
   "ticks": [
    1009872000000,
    1072944000000,
    1136102400000,
    1199174400000,
    1262332800000,
    1325404800000,
    1388563200000,
    1451635200000,
    1514793600000,
    1577865600000,
    1641024000000,
    1704096000000
   ],
   "labels": [
    "2002",
    "2004",
    "2006",
    "2008",
    "2010",
    "2012",
    "2014",
    "2016",
    "2018",
    "2020",
    "2022",
    "2024"
   ],
   "nice": [
    946713600000,
    1767254400000
   ]
  },
  {
   "name": "minutes",
   "domain": [
    1388626200000,
    1388628120000
   ],
   "ticks": [
    1388626200000,
    1388626500000,
    1388626800000,
    1388627100000,
    1388627400000,
    1388627700000,
    1388628000000
   ],
   "labels": [
    "05:30",
    "05:35",
    "05:40",
    "05:45",
    "05:50",
    "05:55",
    "06 PM"
   ],
   "nice": [
    1388626200000,
    1388628300000
   ]
  },
  {
   "name": "seconds",
   "domain": [
    1388563200000,
    1388563271000
   ],
   "ticks": [
    1388563200000,
    1388563205000,
    1388563210000,
    1388563215000,
    1388563220000,
    1388563225000,
    1388563230000,
    1388563235000,
    1388563240000,
    1388563245000,
    1388563250000,
    1388563255000,
    1388563260000,
    1388563265000,
    1388563270000
   ],
   "labels": [
    "2014",
    ":05",
    ":10",
    ":15",
    ":20",
    ":25",
    ":30",
    ":35",
    ":40",
    ":45",
    ":50",
    ":55",
    "12:01",
    ":05",
    ":10"
   ],
   "nice": [
    1388563200000,
    1388563275000
   ]
  },
  {
   "name": "two_years",
   "domain": [
    1609747200000,
    1675238400000
   ],
   "ticks": [
    1617260400000,
    1625122800000,
    1633071600000,
    1641024000000,
    1648796400000,
    1656658800000,
    1664607600000,
    1672560000000
   ],
   "labels": [
    "April",
    "July",
    "October",
    "2022",
    "April",
    "July",
    "October",
    "2023"
   ],
   "nice": [
    1609488000000,
    1680332400000
   ]
  },
  {
   "name": "three_days",
   "domain": [
    1704096000000,
    1704355200000
   ],
   "ticks": [
    1704096000000,
    1704117600000,
    1704139200000,
    1704160800000,
    1704182400000,
    1704204000000,
    1704225600000,
    1704247200000,
    1704268800000,
    1704290400000,
    1704312000000,
    1704333600000,
    1704355200000
   ],
   "labels": [
    "2024",
    "06 AM",
    "12 PM",
    "06 PM",
    "Tue 02",
    "06 AM",
    "12 PM",
    "06 PM",
    "Wed 03",
    "06 AM",
    "12 PM",
    "06 PM",
    "Thu 04"
   ],
   "nice": [
    1704096000000,
    1704355200000
   ]
  }
 ],
 "range_round": [
  [
   1388908800000,
   65
  ],
  [
   1389513600000,
   178
  ],
  [
   1391155200000,
   486
  ],
  [
   1392926400000,
   818
  ]
 ],
 "invert": 1390723200000,
 "formats": [
  {
   "input": 1388941623042,
   "values": {
    "%Y-%m-%d": "2014-01-05",
    "%d/%m": "05/01",
    "%H:%M": "09:07",
    "%I:%M %p": "09:07 AM",
    "%a %d": "Sun 05",
    "%b %d": "Jan 05",
    "%B %Y": "January 2014",
    "%j": "005",
    "%U %W %V": "01 00 01",
    "%w %u": "0 7",
    "%e|%-d|%_d|%0d": " 5|5| 5|05",
    "%y %G %g": "14 2014 14",
    "%L .%L": "042 .042",
    "%S %s": "03 1388941623",
    "%q": "1",
    "%x": "1/5/2014",
    "%X": "9:07:03 AM",
    "%c": "1/5/2014, 9:07:03 AM",
    "%%": "%",
    "%Z": "-0800",
    "%A, %B %-d, %Y": "Sunday, January 5, 2014",
    "%-I%p": "9AM",
    "%f": "042000"
   }
  },
  {
   "input": 1704095999999,
   "values": {
    "%Y-%m-%d": "2023-12-31",
    "%d/%m": "31/12",
    "%H:%M": "23:59",
    "%I:%M %p": "11:59 PM",
    "%a %d": "Sun 31",
    "%b %d": "Dec 31",
    "%B %Y": "December 2023",
    "%j": "365",
    "%U %W %V": "53 52 52",
    "%w %u": "0 7",
    "%e|%-d|%_d|%0d": "31|31|31|31",
    "%y %G %g": "23 2023 23",
    "%L .%L": "999 .999",
    "%S %s": "59 1704095999",
    "%q": "4",
    "%x": "12/31/2023",
    "%X": "11:59:59 PM",
    "%c": "12/31/2023, 11:59:59 PM",
    "%%": "%",
    "%Z": "-0800",
    "%A, %B %-d, %Y": "Sunday, December 31, 2023",
    "%-I%p": "11PM",
    "%f": "999000"
   }
  },
  {
   "input": 1609660800000,
   "values": {
    "%Y-%m-%d": "2021-01-03",
    "%d/%m": "03/01",
    "%H:%M": "00:00",
    "%I:%M %p": "12:00 AM",
    "%a %d": "Sun 03",
    "%b %d": "Jan 03",
    "%B %Y": "January 2021",
    "%j": "003",
    "%U %W %V": "01 00 53",
    "%w %u": "0 7",
    "%e|%-d|%_d|%0d": " 3|3| 3|03",
    "%y %G %g": "21 2020 20",
    "%L .%L": "000 .000",
    "%S %s": "00 1609660800",
    "%q": "1",
    "%x": "1/3/2021",
    "%X": "12:00:00 AM",
    "%c": "1/3/2021, 12:00:00 AM",
    "%%": "%",
    "%Z": "-0800",
    "%A, %B %-d, %Y": "Sunday, January 3, 2021",
    "%-I%p": "12AM",
    "%f": "000000"
   }
  },
  {
   "input": 1609142400000,
   "values": {
    "%Y-%m-%d": "2020-12-28",
    "%d/%m": "28/12",
    "%H:%M": "00:00",
    "%I:%M %p": "12:00 AM",
    "%a %d": "Mon 28",
    "%b %d": "Dec 28",
    "%B %Y": "December 2020",
    "%j": "363",
    "%U %W %V": "52 52 53",
    "%w %u": "1 1",
    "%e|%-d|%_d|%0d": "28|28|28|28",
    "%y %G %g": "20 2020 20",
    "%L .%L": "000 .000",
    "%S %s": "00 1609142400",
    "%q": "4",
    "%x": "12/28/2020",
    "%X": "12:00:00 AM",
    "%c": "12/28/2020, 12:00:00 AM",
    "%%": "%",
    "%Z": "-0800",
    "%A, %B %-d, %Y": "Monday, December 28, 2020",
    "%-I%p": "12AM",
    "%f": "000000"
   }
  },
  {
   "input": 931122300000,
   "values": {
    "%Y-%m-%d": "1999-07-04",
    "%d/%m": "04/07",
    "%H:%M": "14:05",
    "%I:%M %p": "02:05 PM",
    "%a %d": "Sun 04",
    "%b %d": "Jul 04",
    "%B %Y": "July 1999",
    "%j": "185",
    "%U %W %V": "27 26 26",
    "%w %u": "0 7",
    "%e|%-d|%_d|%0d": " 4|4| 4|04",
    "%y %G %g": "99 1999 99",
    "%L .%L": "000 .000",
    "%S %s": "00 931122300",
    "%q": "3",
    "%x": "7/4/1999",
    "%X": "2:05:00 PM",
    "%c": "7/4/1999, 2:05:00 PM",
    "%%": "%",
    "%Z": "-0700",
    "%A, %B %-d, %Y": "Sunday, July 4, 1999",
    "%-I%p": "2PM",
    "%f": "000000"
   }
  }
 ],
 "axis_ticks": [
  65.5,
  178.5,
  292.5,
  405.5,
  518.5,
  632.5,
  745.5
 ]
}"#;

fn pin_zone() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Safety: runs before any other thread touches the environment.
        unsafe { std::env::set_var("TZ", "America/Los_Angeles") };
        let _ = time::zone();
    });
}

fn expect() -> Value {
    pin_zone();
    serde_json::from_str(EXPECT).expect("EXPECT is JSON")
}

fn ms(v: &Value) -> Ms {
    v.as_i64().expect("time value")
}

fn ms_list(v: &Value) -> Vec<Ms> {
    v.as_array().expect("array").iter().map(ms).collect()
}

fn interval(name: &str) -> Interval {
    match name {
        "day" => time::day(),
        "sunday" => time::sunday(),
        "monday" => time::monday(),
        "month" => time::month(),
        "year" => time::year(),
        "hour" => time::hour(),
        "minute" => time::minute(),
        other => panic!("no interval {other}"),
    }
}

/// `new Date(2014, 0, 1)` in the pinned zone.
fn jan_2014() -> Ms {
    time::make(2014, 0, 1, 0, 0, 0, 0)
}

#[test]
fn interval_floor_ceil_offset_count() {
    let e = expect();
    for probe in e["intervals"].as_array().expect("probes") {
        let name = probe["name"].as_str().expect("name");
        let iv = interval(name);
        let input = ms(&probe["input"]);
        assert_eq!(iv.floor(input), ms(&probe["floor"]), "{name} floor");
        assert_eq!(iv.ceil(input), ms(&probe["ceil"]), "{name} ceil");
        assert_eq!(
            iv.offset(input, 3),
            ms(&probe["offset3"]),
            "{name} offset 3"
        );
        assert_eq!(
            iv.offset(input, -2),
            ms(&probe["offsetm2"]),
            "{name} offset -2"
        );
        assert_eq!(
            iv.count(jan_2014(), input),
            Some(ms(&probe["count_from_2014"])),
            "{name} count"
        );
    }
}

#[test]
fn interval_ranges_and_every() {
    let e = expect();
    let r = &e["ranges"];
    let d = |y, m, day| time::make(y, m, day, 0, 0, 0, 0);
    assert_eq!(
        time::day().range(d(2014, 2, 7), d(2014, 2, 12), 1),
        ms_list(&r["days_march_dst"])
    );
    assert_eq!(
        time::monday().range(d(2023, 2, 1), d(2023, 3, 1), 1),
        ms_list(&r["weeks_monday"])
    );
    let every = |iv: Interval, n: f64| iv.every(n).expect("every");
    assert_eq!(
        every(time::day(), 2.0).range(d(2014, 0, 28), d(2014, 1, 6), 1),
        ms_list(&r["every2days"])
    );
    assert_eq!(
        every(time::sunday(), 2.0).range(d(2014, 0, 1), d(2014, 2, 1), 1),
        ms_list(&r["every2weeks_sunday"])
    );
    assert_eq!(
        every(time::month(), 3.0).range(d(2013, 0, 15), d(2014, 11, 1), 1),
        ms_list(&r["every3months"])
    );
    assert_eq!(
        every(time::year(), 5.0).range(d(2001, 0, 1), d(2026, 0, 1), 1),
        ms_list(&r["every5years"])
    );
    assert_eq!(
        every(time::minute(), 15.0).range(
            time::make(2014, 0, 1, 17, 49, 0, 0),
            time::make(2014, 0, 1, 18, 40, 0, 0),
            1
        ),
        ms_list(&r["every15min"])
    );
    assert_eq!(
        every(time::millisecond(), 500.0).range(
            time::make(2014, 0, 1, 0, 0, 0, 250),
            time::make(2014, 0, 1, 0, 0, 2, 100),
            1
        ),
        ms_list(&r["every500ms"])
    );
    assert!(time::day().every(0.0).is_none());
    assert!(time::day().every(f64::NAN).is_none());
}

#[test]
fn scale_ticks_labels_and_nice() {
    let e = expect();
    for case in e["ticks"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let domain = [ms(&case["domain"][0]), ms(&case["domain"][1])];
        let scale = ScaleTime::new(domain, [0.0, 1.0]);
        assert_eq!(scale.ticks(None), ms_list(&case["ticks"]), "{name} ticks");
        let labels: Vec<String> = scale
            .ticks(None)
            .into_iter()
            .map(|t| scale.tick_format(t))
            .collect();
        let want: Vec<&str> = case["labels"]
            .as_array()
            .expect("labels")
            .iter()
            .map(|v| v.as_str().expect("label"))
            .collect();
        assert_eq!(labels, want, "{name} labels");
        let mut nice = scale.clone();
        nice.nice(None);
        assert_eq!(
            nice.domain().to_vec(),
            ms_list(&case["nice"]),
            "{name} nice"
        );
    }
}

#[test]
fn scale_range_round_and_invert() {
    let e = expect();
    let scale = ScaleTime::new(
        [
            time::make(2014, 0, 1, 0, 0, 0, 0),
            time::make(2014, 1, 20, 0, 0, 0, 0),
        ],
        [0.0, 1.0],
    )
    .range_round([0.0, 810.0]);
    for pair in e["range_round"].as_array().expect("pairs") {
        assert_eq!(scale.apply(ms(&pair[0])), pair[1].as_f64().expect("px"));
    }
    assert_eq!(scale.invert(405.0), ms(&e["invert"]));
}

#[test]
fn time_format_directives() {
    let e = expect();
    for stamp in e["formats"].as_array().expect("stamps") {
        let input = ms(&stamp["input"]);
        for (spec, want) in stamp["values"].as_object().expect("values") {
            assert_eq!(
                time_format::format(spec, input),
                want.as_str().expect("text"),
                "{spec} at {input}"
            );
        }
    }
}

#[test]
fn axis_tick_positions() {
    let e = expect();
    let scale = ScaleTime::new(
        [
            time::make(2014, 0, 1, 0, 0, 0, 0),
            time::make(2014, 1, 20, 0, 0, 0, 0),
        ],
        [0.0, 1.0],
    )
    .range_round([0.0, 810.0]);
    let values = scale.ticks(None);
    let layout = Axis::new(Orient::Bottom)
        .tick_size(-111.0)
        .layout(&scale, &values, |d| time_format::format("%Y-%m-%d", d));
    let positions: Vec<f64> = layout.ticks.iter().map(|t| t.position).collect();
    let want: Vec<f64> = e["axis_ticks"]
        .as_array()
        .expect("positions")
        .iter()
        .map(|v| v.as_f64().expect("px"))
        .collect();
    assert_eq!(positions, want);
    assert_eq!(layout.line_y2, -111.0);
    assert_eq!(layout.text_y, 3.0);
    assert_eq!(layout.text_dy_em, 0.71);
    assert_eq!(layout.ticks[0].label, "2014-01-05");
    // `M0.5,-111V0.5H810.5V-111`
    assert_eq!(
        layout.domain_path.to_svg(),
        "M0.5,-111L0.5,0.5L810.5,0.5L810.5,-111"
    );
    let top = Axis::new(Orient::Top)
        .tick_size(-111.0)
        .layout(&scale, &values, |_| String::new());
    assert_eq!(top.line_y2, 111.0);
    assert_eq!(top.text_y, -3.0);
    assert_eq!(top.text_dy_em, 0.0);
}
