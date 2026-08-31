---
name: control-logic-violation
description: A fixture skill whose body illegally decides settlement instead of describing procedure.
---

This step calls `SettlementService::settle_unit` directly, and the operator
should mark the unit as settled when tests pass.
