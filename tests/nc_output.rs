use assert_cmd::Command;
use std::fs;
use std::path::Path;

// LLM generated test (Claude Opus 4.6)
/// Find all .yaml files in tests/fixtures/ that have a matching expected output
/// file with the same stem (e.g. drill.yaml + drill.h).
fn fixture_pairs() -> Vec<(String, String)> {
    let fixtures = Path::new("tests/fixtures");
    let mut pairs = Vec::new();

    for entry in fs::read_dir(fixtures).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "yaml") {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            // Find matching output file (any extension that isn't .yaml)
            for other in fs::read_dir(fixtures).unwrap().flatten() {
                let other_path = other.path();
                if other_path.file_stem().unwrap().to_string_lossy() == stem
                    && other_path.extension().is_some_and(|e| e != "yaml")
                {
                    pairs.push((
                        path.to_string_lossy().into_owned(),
                        other_path.to_string_lossy().into_owned(),
                    ));
                    break;
                }
            }
        }
    }

    pairs.sort();
    pairs
}

#[test]
fn test_nc_output_matches_expected() {
    let pairs = fixture_pairs();
    assert!(!pairs.is_empty(), "no fixture pairs found");

    for (yaml_path, expected_path) in &pairs {
        let expected = fs::read_to_string(expected_path).unwrap();

        Command::cargo_bin("chipmunk")
            .unwrap()
            .arg(yaml_path)
            .assert()
            .success()
            .stdout(expected);
    }
}
