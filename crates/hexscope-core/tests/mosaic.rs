use hexscope_core::mosaic::{Signal, analyze};
fn signal(
    index: usize,
    serial: Option<&str>,
    location: Option<[f64; 2]>,
    taken: Option<&str>,
) -> Signal {
    Signal {
        index,
        serial: serial.map(String::from),
        location,
        taken: taken.map(String::from),
    }
}
#[test]
fn repeated_facts_export_only_indexes() {
    let report = analyze(&[
        signal(
            1,
            Some(" private-serial "),
            Some([50.0, 30.0]),
            Some("2024:01:02 10:30:00"),
        ),
        signal(
            2,
            Some("private-serial"),
            Some([50.0001, 30.0]),
            Some("2024:01:01 10:30:00"),
        ),
        signal(3, Some("unknown"), None, Some("2024:02:31 10:00:00")),
    ])
    .unwrap();
    assert_eq!(report.serial_groups, vec![vec![1, 2]]);
    assert_eq!(report.location_groups, vec![vec![1, 2]]);
    assert_eq!(report.local_date_order, vec![2, 1]);
    let json = report.to_json();
    assert!(!json.contains("private-serial"));
    assert!(!json.contains("50.0001"));
    assert!(!json.contains("2024:"));
}
#[test]
fn spherical_distance_handles_date_line_and_poles() {
    let report = analyze(&[
        signal(1, None, Some([0.0, 179.9999]), None),
        signal(2, None, Some([0.0, -179.9999]), None),
        signal(3, None, Some([89.9999, 40.0]), None),
        signal(4, None, Some([89.9999, -40.0]), None),
        signal(5, None, Some([91.0, 0.0]), None),
    ])
    .unwrap();
    assert_eq!(report.location_groups, vec![vec![1, 2], vec![3, 4]]);
    assert_eq!(report.located_files, 4);
}
#[test]
fn never_merges_a_chain_of_distant_locations_and_normalizes_known_offsets() {
    let report = analyze(&[
        signal(1, None, Some([0.0, 0.0]), Some("2024-01-01T12:00:00+02:00")),
        signal(2, None, Some([0.0, 0.0008]), Some("2024-01-01T10:30:00Z")),
        signal(3, None, Some([0.0, 0.0016]), None),
    ])
    .unwrap();
    assert_eq!(report.location_groups, vec![vec![1, 2]]);
    assert_eq!(report.utc_date_order, vec![1, 2]);
    assert!(analyze(&vec![signal(1, None, None, None); 1001]).is_err());
}
