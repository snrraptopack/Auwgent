use super::orchestrator::extract_yaml;

#[test]
fn test_extract_yaml_robustness() {
    let cases = vec![
        (
            "Just yaml:\nresponse: ok",
            "response: ok"
        ),
        (
            "With fences:\n```yaml\nresponse: ok\n```",
            "response: ok"
        ),
        (
            "Unclosed fence:\n```yaml\nresponse: ok",
            "response: ok"
        ),
        (
            "Multiple fences:\n```yaml\nfirst: 1\n```\nSome noise\n```yaml\nsecond: 2\n```",
            "first: 1\n\nsecond: 2"
        ),
        (
            "Fences with noise:\nNoise before\n```\nresponse: ok\n```\nNoise after",
            "response: ok"
        ),
        (
            "Brittle case from issue:\n{}\n```yaml\nresponse: ok\n```",
            "response: ok"
        )
    ];

    for (input, expected) in cases {
        let actual = extract_yaml(input);
        assert_eq!(actual.trim(), expected.trim(), "Failed for input: {}", input);
    }
}
