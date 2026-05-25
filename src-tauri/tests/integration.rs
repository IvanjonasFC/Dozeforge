//! Integration tests against the shipped fixtures.

use std::fs;
use std::path::PathBuf;

use dozeforge_lib::heuristics::proxy_detector::rank;
use dozeforge_lib::parsers::alarm::AlarmParser;
use dozeforge_lib::parsers::batterystats::BatteryStatsParser;
use dozeforge_lib::parsers::jobscheduler::JobSchedulerParser;
use dozeforge_lib::parsers::Parser;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

#[test]
fn batterystats_api34_aggregates_pwl_by_uid() {
    let input = fixture("batterystats_api34.txt");
    let parser = BatteryStatsParser::for_api(34);
    let out = parser.parse(&input).expect("parse ok");
    assert!(!out.is_empty());
    let spam = out.iter()
        .find(|e| e.package.as_str() == "com.example.spammer")
        .expect("spammer present");
    assert!(spam.total_ms > 5_000_000);
    assert!(spam.count >= 600);
}

#[test]
fn alarm_api34_credits_owner_not_target() {
    let input = fixture("dumpsys_alarm_api34.txt");
    let parser = AlarmParser;
    let out = parser.parse(&input).expect("parse ok");
    let spam = out.iter().find(|a| a.triggering_package.as_str() == "com.example.spammer");
    assert!(spam.is_some(), "spammer must be present");
    assert!(spam.unwrap().wake_count >= 2);
}

#[test]
fn jobscheduler_api34_counts_by_package() {
    let input = fixture("dumpsys_jobscheduler_api34.txt");
    let parser = JobSchedulerParser;
    let out = parser.parse(&input).expect("parse ok");
    let spam = out.iter()
        .find(|j| j.package.as_str() == "com.example.spammer")
        .expect("spammer present");
    assert_eq!(spam.job_count, 3);
    assert_eq!(spam.periodic_count, 2);
}

#[test]
fn end_to_end_culprit_ranking_redirects_gms_proxy() {
    let bs = BatteryStatsParser::for_api(34)
        .parse(&fixture("batterystats_api34.txt"))
        .unwrap();
    let alarms = AlarmParser.parse(&fixture("dumpsys_alarm_api34.txt")).unwrap();
    let jobs = JobSchedulerParser
        .parse(&fixture("dumpsys_jobscheduler_api34.txt"))
        .unwrap();

    let ranking = rank(&bs, &alarms, &jobs);
    let top = ranking.first().expect("ranking non-empty");
    assert_eq!(top.package.as_str(), "com.example.spammer");
}
