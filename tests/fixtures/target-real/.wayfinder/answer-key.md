## AK-951
claim: The gadget source declares the stable fixture marker GADGET_MARKER_REAL_V1.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/real-marker-report.json, producer: instrument}

## AK-952
claim: The project's real check command runs the gadget check and exits zero under live containment.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: evidence/real-check-report.json, producer: instrument}

## AK-954
claim: The gadget source carries the stable fixture marker GADGET_MARKER_REAL_V1 and the check command exits zero over the local-tier dispatch workspace.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: evidence/real-local-report.json, producer: instrument}

## AK-953
claim: The fixture's irreversible demand — the archived release is destroyed — is a change only a human may authorize.
verify_by: static_check
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: evidence/real-irreversible-report.json, producer: instrument}
