## AK-961
claim: The gadget source preserves the baseline marker BASELINE_MARKER_PRESERVED_V1 and adds the baseline feature marker BASELINE_FEATURE_ADDED_V1.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/baseline-marker-report.json, producer: instrument}

## AK-962
claim: The gadget source preserves the baseline marker BASELINE_MARKER_PRESERVED_V1 and adds the baseline feature marker BASELINE_FEATURE_ADDED_V1, and the declared check exits zero over the dispatch workspace.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/baseline-feature-report.json, producer: instrument}
