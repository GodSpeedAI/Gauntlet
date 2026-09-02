## AK-901
claim: The gadget source declares the stable fixture marker GADGET_MARKER_V1.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/marker-check-report.json, producer: instrument}

## AK-902
claim: The project's real check command runs the gadget check and exits zero.
verify_by: build
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/check-report.json, producer: instrument}

## AK-903
claim: The project's check report records at least one executed check and zero failures.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/full-check-report.json, producer: instrument}
