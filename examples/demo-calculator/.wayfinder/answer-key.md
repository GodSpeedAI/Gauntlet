## AK-9011
claim: calculator.html exists at the workspace root and declares digit buttons 0-9 and operator buttons for add, subtract, multiply, and divide.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/markup-check-report.json, producer: instrument}

## AK-9012
claim: The project's real check command runs the calculator check and exits zero.
verify_by: build
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/check-report.json, producer: instrument}

## AK-9013
claim: calculator.html's evaluation path produces correct results for at least two arithmetic expressions covering different operators.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: evidence/eval-check-report.json, producer: instrument}
