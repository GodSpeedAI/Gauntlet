# Gauntlet

### Keep long-running agent work from turning into a persuasive blur.

One agent call is easy to understand.

You give it a task. It works. You inspect the result.

Now let it work for an hour.

It reads dozens of files. Changes its plan. Calls tools. Retries failures. Delegates to another agent. Discovers a requirement it missed. Rewrites three files. Runs some tests. Runs out of context. Resumes. Fixes the fix.

Eventually it says:

> Done.

At that point, the hard question is no longer whether the model is smart.

It is:

> **What kept the target fixed while the work changed underneath it?**

**Gauntlet is a constrained execution harness for long-horizon agentic work. It turns a bounded work contract into an orchestrated sequence of agent and tool actions, checks, recovery, and evidence while keeping the governing objective and acceptance criteria outside the executor.**

Plainly: Gauntlet gives agents room to work for a long time without letting the work gradually redefine what counts as success.

Its basic loop is:

```text
bounded work contract
        ↓
     execute
        ↓
     observe
        ↓
     compare
        ↓
      verify
        ↓
   correct if needed
        ↓
      settle
        ↓
 durable evidence
```

Agents can plan, search, code, delegate, retry, and recover inside that loop.

They do not get to move the ruler because the ruler became inconvenient.

---

## Long-horizon agents create a different kind of failure

A short agent task can fail obviously.

The code does not compile. The answer is wrong. The tool call errors.

Long-running work can fail while looking much healthier.

Suppose you ask an agent to implement account lockout after repeated failed logins.

Twenty minutes later it has:

* edited the authentication service
* changed a schema
* added tests
* updated configuration
* delegated part of the work
* fixed a regression
* replaced an earlier implementation
* summarized the result

Every individual step may look plausible.

The original requirement may still be wrong.

Maybe lockout resets when the service restarts.

Maybe the agent tested three failures when the requirement said five.

Maybe the test it cites never ran after the final correction.

Maybe a delegated agent changed a neighboring policy.

Maybe the implementation works, but only because the acceptance criterion quietly became easier halfway through the run.

The longer the work continues, the easier it becomes for these distinctions to disappear into the history of the run.

A bigger context window does not solve that.

More agents do not solve it.

A more capable model does not solve it.

The run needs structure outside the model.

---

## Start with one task you normally supervise too closely

The easiest way to understand Gauntlet is to give it work you would otherwise keep checking manually.

For example:

```bash
gauntlet run "Implement account lockout after repeated failed logins"
```

The interesting part is not that an agent starts coding.

Before execution becomes meaningful, Gauntlet needs a stable account of what the finished work must satisfy.

During the run it needs independent observations of what actually exists.

When the implementation differs from the requirement, it needs to materialize that difference instead of allowing it to disappear into another round of prompting.

When evidence is missing, it needs to keep that uncertainty visible.

When a correction fails, it needs to recover without forgetting everything already established.

And when the work finally stops, it needs to stop for a reason stronger than:

```text
the agent sounded finished
```

That is the experiment.

---

## The Reference keeps the run pointed at the same thing

Every Gauntlet run works against a **Reference**.

The Reference defines what the finished result is supposed to satisfy:

```text
required behavior
constraints
invariants
forbidden outcomes
required evidence
acceptance conditions
```

Before execution, Gauntlet resolves and freezes that Reference.

During the run, the executor works against it.

At settlement, the Reference is checked again.

If the work becomes difficult, the executor cannot quietly weaken the Reference and then claim success against the easier version.

This failure is common even without AI.

A team begins with:

```text
response time must remain below 200 ms
```

The implementation struggles.

Later the requirement becomes:

```text
response time should usually be reasonable
```

Eventually the project passes.

Nothing fraudulent necessarily happened. Pressure gradually changed the ruler.

Agent systems can do the same thing much faster.

Gauntlet makes changing the Reference an explicit event instead of an accidental side effect of execution.

If the Reference itself is wrong, change it deliberately.

Do not let the executor rewrite the criterion its own work is being measured against.

---

## References become individually falsifiable claims

A large requirement is difficult to verify as one blob.

Gauntlet decomposes it into checkable **Units**.

A Unit might look like:

```yaml
id: AUTH-014
claim: accounts lock after five consecutive failed logins
verify_by: test
blast_radius: local
authority: critic

evidence_required:
  kind: test_report
  locator: tests/auth/lockout.rs::locks_after_five_failures
  producer: instrument
```

A Unit is not merely another task.

It is a claim about the resulting system that can be shown to hold, shown to fail, or remain unresolved.

That difference matters.

This:

```text
Task:
Implement lockout.
```

describes work.

This:

```text
Claim:
After five consecutive failed logins,
the account enters the locked state.

Evidence:
integration test X
```

describes something reality can push back against.

Gauntlet executes work in service of those claims.

---

## The builder does not get to observe itself into success

Most agent loops have one perspective:

```text
agent acts
agent reads result
agent interprets result
agent decides whether result is good
agent continues or declares completion
```

That is efficient.

It is also easy for the same mistake to survive every stage.

Gauntlet separates roles.

```text
Executor
changes the target.

Observer
records what actually exists.

Comparator
finds material differences from the Reference.

Verifier
checks specific claims against required evidence.
```

These roles may ultimately be implemented by models, tools, people, or deterministic programs.

What matters is that they are different interfaces.

The comparator does not need the builder's internal story about why the work should be accepted.

It receives the things relevant to comparison:

```text
frozen Reference
observed target state
instrument evidence
comparison contract
```

A persuasive explanation from the executor cannot repair a missing test result because the explanation is not the test result.

---

## Observation, comparison, and verification are different jobs

Consider this requirement:

```text
Past dates must be rejected.
```

An observer might inspect the implementation and produce:

```text
Date validation exists in request.rs.
```

That is an observation.

The comparator might say:

```text
The validation appears to reject dates before today.
No residual detected in the implementation.
```

That is a comparison.

The verifier might run:

```text
tests/leave/date_validation.rs::rejects_past_dates
```

and receive:

```text
PASS
```

That is verification.

Those statements have different evidentiary strength.

Gauntlet keeps them separate.

A model is extremely useful for finding suspicious differences.

Where harder evidence exists, the hard evidence gets the final word.

---

## Verification has three answers

Gauntlet does not force every question into true or false before enough evidence exists.

A verification result can be:

| Result           | Meaning                                                       |
| ---------------- | ------------------------------------------------------------- |
| **PASS**         | Required evidence was obtained and supports the claim.        |
| **FAIL**         | Required evidence was obtained and contradicts the claim.     |
| **INCONCLUSIVE** | The evidence needed to decide is unavailable or insufficient. |

That third state is important.

Suppose the integration environment is down.

The required test cannot run.

The claim is not therefore false.

It is also not therefore true.

It is unresolved.

The same applies when:

* a tool times out
* an expected artifact is missing
* a reviewer lacks enough evidence
* a dependency is unavailable
* an observation cannot be attributed reliably

Gauntlet keeps the uncertainty instead of rounding it toward completion.

---

## One command returning zero does not settle a long run

Execution success and work acceptance are different claims.

This:

```text
cargo test
exit code: 0
```

shows that a particular test command completed successfully.

It does not automatically establish:

```text
the right tests ran
the final artifact is correct
all required Units were checked
nothing important regressed
the Reference remained unchanged
the work is ready to accept
```

Gauntlet settles against the frozen work contract and the evidence accumulated during the run.

The loop is closer to:

```text
Reference
    ↓
execution
    ↓
observation
    ↓
residuals
    ↓
verification
    ↓
all required claims supported?
    ├── no  → correct
    ├── ?   → gather evidence / escalate / stop inconclusive
    └── yes → settle
```

The executor finishing is an event inside the run.

It is not the definition of completion.

---

## Corrections should close the observed gap

A common agent recovery strategy is:

```text
something failed
      ↓
give the model more context
      ↓
rewrite more code
      ↓
try again
```

Sometimes that works.

Sometimes the correction creates three new variables while fixing one.

Gauntlet chooses corrections from the residual that was actually observed.

A correction can consider:

```text
what failed
what evidence supports the diagnosis
what remains unresolved
what authority is available
what budget remains
what blast radius is acceptable
what new observation would discriminate between explanations
```

The target is the smallest correction that could plausibly close the gap.

For example:

```text
Observed:
one integration test fails because an existing cache
preserves the old authentication state.

Bad correction:
rewrite the authentication subsystem.

Better correction:
fix cache invalidation,
rerun the discriminating test,
reobserve.
```

The goal is not minimal code for its own sake.

The goal is to spend as little additional uncertainty and blast radius as necessary to learn whether the diagnosed problem was actually the problem.

---

## A failed correction can still buy useful information

Long-running agent systems become expensive when failure simply causes another opaque attempt.

Gauntlet keeps the sequence:

```text
expectation
attempt
observation
difference
correction
new observation
```

That lets the next move depend on what the previous attempt actually taught.

A failed correction may establish:

```text
the diagnosis was wrong
the Reference is underspecified
a dependency is missing
the problem lies outside the current scope
the required capability is unavailable
the remaining work is unaffordable under the current budget
```

That is still movement if the run preserves the distinction.

Repeating the same error with the same representation and the same correction is different.

Gauntlet is designed to make that visible.

---

## Every run has a budget

An agent that can continue forever can turn almost any failure into a large bill.

Gauntlet bounds execution across surfaces such as:

```text
wall time
iterations
tool calls
model spend
cost
parallelism
workspace scope
authority
```

If a useful next move no longer fits the remaining budget, the correct response may be:

```text
BLOCKED
BUDGET_EXHAUSTED
ESCALATED
INCONCLUSIVE
```

instead of another attempt.

A run can terminate as:

```text
SETTLED
BLOCKED
BUDGET_EXHAUSTED
ESCALATED
ABORTED
INCONCLUSIVE
```

A bounded failure is a legitimate result.

An unbounded loop that eventually manufactures something called success is harder to trust.

---

## Recovery is part of the execution model

Agent work is not reliable because nothing goes wrong.

It becomes more useful when interruption does not destroy the useful state accumulated before the interruption.

Tools fail.

Processes crash.

Models return malformed output.

Workspaces get dirty.

Machines restart.

Remote systems disappear.

Gauntlet treats recovery as part of normal execution.

A run retains enough durable state to answer:

```text
Which Reference was frozen?
Which Units are already settled?
Which residuals remain open?
Which invocations already happened?
What evidence already exists?
How much budget has been spent?
What work was in flight?
What should happen next?
```

The answer comes from records.

Not from:

> "Remind me what we were doing."

---

## State does not live in the chat transcript

Every consequential transition is written to durable state, evidence, and an append-only event history.

That matters once runs become longer than one model context.

A resumed run can reconstruct its working state from recorded events rather than depending on the executor's memory of the conversation.

Conceptually:

```text
Session
  ↓
Run
  ↓
Task / Unit / Invocation
  ↓
events + evidence + state transitions
```

The transcript may still be useful evidence.

It is not the state machine.

That difference is what allows a run to survive process restarts, model changes, context truncation, and executor replacement.

---

## Executors are replaceable

Gauntlet is built around roles rather than one model vendor or one agent product.

Core interfaces include roles such as:

```text
AgentRunner
Observer
Comparator
Verifier
ReferenceProvider
PolicyProvider
StateStore
EvidenceStore
EventLog
Sandbox
BudgetMeter
```

Adapters can sit behind them:

```text
CLI coding agents
model APIs
test runners
Git
browsers
SQLite
filesystems
sandboxes
observability systems
```

The control loop should not need to change because the executor changed.

A synchronous CLI agent may block until it finishes.

Another harness may start a task asynchronously and require polling.

Gauntlet can normalize those different execution styles into lifecycle states such as:

```text
running
completed
failed
timed_out
cancelled
```

The executor is a participant in the run.

It is not the architecture.

---

## More agents do not change the contract

Parallelism can help.

One agent can investigate an implementation while another examines tests and another searches for regressions.

That does not mean each branch gets to invent its own definition of success.

A Gauntlet run can coordinate:

```text
implementation agents
sub-agents
critics
independent verifiers
deterministic tools
parallel search
specialized skills
```

while holding one governing work contract outside those executors.

Conceptually:

```text
                 frozen Reference
                       |
          +------------+------------+
          |            |            |
       agent A       agent B      tool
          |            |            |
          +------------+------------+
                       |
                    observe
                       |
                    compare
                       |
                    verify
```

Parallel execution increases the need for a stable comparison boundary.

It does not reduce it.

---

## Formation keeps unknown work from pretending to be specified work

Sometimes the run does not yet have a trustworthy Reference.

That is a real state.

Gauntlet calls it **formation**.

Before dispatching execution, it can inspect available context and determine whether the desired transformation is sufficiently represented to support a frozen work contract.

If the system knows:

```text
what must change
what must remain true
what evidence can establish the result
what important unknowns remain
```

the Reference can be formed and frozen.

If it cannot establish those things without guessing, the uncertainty remains visible.

That prevents an agent from receiving a vague destination such as:

```text
"make the authentication system enterprise ready"
```

and silently inventing the criteria by which its own work will later be judged.

Representation comes before long execution when execution depends on those distinctions.

---

## Authority stays outside the executor

Gauntlet constrains execution.

It does not gain authority merely because it can orchestrate powerful agents.

There are two distinct questions:

```text
May this work happen?

How should allowed work be executed?
```

The first is authority.

The second is Gauntlet's execution problem.

A Gauntlet deployment can consume decisions through a `PolicyProvider`, approval surface, or other authority integration.

Inside the broader GodSpeed architecture, SEA-Forge owns the runtime authority boundary:

```text
proposed work
     ↓
SEA-Forge
allow / deny / escalate
     ↓
authorized work contract
     ↓
Gauntlet
constrained execution
```

A sandbox is also separate.

Authority determines whether work may begin.

Containment determines the environment in which allowed work can proceed.

Gauntlet can build and use bounded execution environments without treating containment as permission.

---

## The Reference can come from a semantic model

A work contract does not have to begin as prose.

A `.sea` model can provide canonical domain meaning for the system being changed.

SEA stands for **Semantic Executable Abstraction**.

DomainForge compiles `.sea` into a canonical semantic model that can expose:

```text
entities
resources
flows
roles
relations
policies
metrics
constraints
concept identities
```

That gives Gauntlet a stronger starting point than repeatedly asking each executor to reconstruct the domain from scattered repository artifacts.

For example:

```text
.sea domain model
      ↓
DomainForge
      ↓
validated semantic representation
      ↓
Gauntlet formation
      ↓
frozen work Reference
      ↓
execution
```

The domain model does not automatically tell Gauntlet how to implement a task.

It gives the run explicit meaning against which implementation choices can be constrained and checked.

---

## One settled run is not durable capability

Gauntlet can produce strong evidence that a particular work contract settled.

That claim should stay narrow.

This:

```text
Run 481 settled successfully.
```

does not automatically justify:

```text
The system now has a reliable capability
for this entire class of work.
```

Durable capability requires more:

```text
repetition
variation
transfer
reduced orchestration burden
recovery after disruption
continued verification quality
```

Gauntlet therefore preserves developmental evidence such as:

```text
attempts
failures
residuals
corrections
payments
recoveries
settlements
```

That history can support later promotion decisions.

In the broader GodSpeed architecture, GodSpeed-Agent owns the stronger developmental question:

> What does this settlement history justify making easier or more directly spendable next?

Gauntlet supplies the execution evidence.

It does not turn one good run into an imaginary muscle.

---

## Faster can be worse

Suppose a later version of a route completes the same work in half the time.

That looks like improvement.

Now suppose it became faster by:

```text
skipping an integration test
removing an independent comparison
reducing variation coverage
ignoring a recovery path
accepting missing evidence as success
```

The route became cheaper by deleting the thing that made its result trustworthy.

Gauntlet's developmental records make that kind of compression inspectable.

A shortcut is only useful when the evidence standard survives it.

The goal is not:

```text
fewer steps
```

The goal is:

```text
lower burden
while preserving the properties
that made settlement meaningful
```

---

## Gauntlet can evaluate changes to Gauntlet

A system that improves itself creates an awkward problem.

If version B changes its own rules and then evaluates itself using those new rules, nearly any improvement can be made to look successful.

Gauntlet treats self-revision like other consequential work.

A candidate change can go through:

```text
proposal
   ↓
isolated implementation
   ↓
historical replay
   ↓
held-out evaluation
   ↓
regression checks
   ↓
protected invariants
   ↓
promotion or rejection
```

The candidate does not become the active runtime merely because the candidate prefers itself.

If the revision regresses an established property, rollback remains available.

This creates a path toward improving the harness without making production the experiment.

---

## What running Gauntlet looks like

Initialize or inspect the environment:

```bash
gauntlet doctor
gauntlet init
```

Work with References:

```text
gauntlet reference form
gauntlet reference freeze
gauntlet reference show
gauntlet reference digest
gauntlet reference diff
gauntlet reference verify
```

Start work:

```bash
gauntlet run "Implement account lockout after repeated failed logins"
```

Inspect and continue it:

```text
gauntlet status
gauntlet inspect
gauntlet verify
gauntlet resume <run_id>
```

Inspect the surrounding runtime:

```text
gauntlet escalations
gauntlet roles
gauntlet skills
```

Use accumulated evidence:

```text
gauntlet improve
gauntlet export
```

The CLI is the reference operator surface.

A TUI, IDE integration, dashboard, or other view can project the same runtime state without becoming the control system itself.

---

## The execution architecture

At a high level:

```text
Operator
   ↓
bounded work request
   ↓
Reference formation / loading
   ↓
Reference freeze
   ↓
RunController
   ↓
IterationEngine
   ↓
UnitScheduler
   ↓
AgentRunner / tools
   ↓
Observer
   ↓
Comparator
   ↓
Verifier
   ↓
CorrectionSelector
   ↓
SettlementService
   ↓
durable state + evidence + events
```

The runtime talks to implementations through ports:

```text
ReferenceProvider
AgentRunner
Observer
Comparator
Verifier
PolicyProvider
StateStore
EvidenceStore
EventLog
Sandbox
BudgetMeter
```

Specific tools remain adapters.

That boundary lets the orchestration strategy evolve without making the correctness model depend on whichever agent product happens to be fashionable this month.

---

## What Gauntlet keeps separate

Several distinctions carry most of the system.

### Reference ≠ implementation

What should be true is not the code that currently exists.

### Execution ≠ observation

The thing making the change does not get exclusive authority to describe the resulting state.

### Observation ≠ comparison

Recording reality and evaluating its difference from the Reference are different operations.

### Comparison ≠ verification

A plausible residual is not yet a proven failure.

A plausible absence of residuals is not yet a proven pass.

### Missing evidence ≠ passing evidence

Unknown stays unknown.

### Authority ≠ containment

Permission to act and limits on how execution occurs answer different questions.

### Execution ≠ settlement

A process finishing does not establish that the required outcome occurred.

### Settlement ≠ capability

One accepted run does not establish reliable future performance.

### Developmental evidence ≠ production promotion

History can propose a shortcut.

It does not authorize the shortcut by itself.

---

## Structural invariants

Gauntlet is designed to preserve these properties regardless of executor:

| Invariant                                                                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------------------- |
| The executor cannot modify the frozen standard its own work is judged against during the run.                                            |
| Observation cannot collapse into the executor's completion claim.                                                                        |
| Comparison and verification remain separate roles.                                                                                       |
| Missing required evidence cannot become a pass.                                                                                          |
| A model judgment may propose a correction but cannot substitute for deterministic proof where the contract requires deterministic proof. |
| Every run remains bounded.                                                                                                               |
| Resumption reconstructs state from durable records rather than conversational memory.                                                    |
| One settled run does not establish durable capability.                                                                                   |
| Authority remains external to execution.                                                                                                 |
| Containment cannot substitute for authority.                                                                                             |
| Executor implementations remain replaceable behind stable roles.                                                                         |
| Self-revision is evaluated before promotion rather than becoming active by self-declaration.                                             |

Where these properties can be enforced through interfaces and state transitions, Gauntlet should enforce them there rather than relying on prompt wording.

---

## What Gauntlet is not

Gauntlet is not an LLM.

It is not a prompt pack.

It is not a collection of agent personas.

It is not a model router.

It is not a code-review bot.

It is not a test runner.

It is not a task tracker.

It is not one fixed multi-agent graph.

It is not the authority layer.

And it is not a mechanism for converting unlimited recursive autonomy into safety.

Models, test runners, critics, sandboxes, agents, and workflow systems can all participate inside a Gauntlet run.

Gauntlet is the constrained execution harness that determines how that work progresses, gets observed, corrected, recovered, and returned with evidence.

---

## Where it fits

Gauntlet can run standalone.

Inside the broader GodSpeed architecture, each neighboring component owns a different boundary.

```text
.sea
represents consequential domain meaning
        ↓
DomainForge
compiles and validates that meaning
        ↓
Context Kernel
supplies bounded relevant context
        ↓
SEA-Forge
authorizes consequential action
        ↓
Gauntlet
executes difficult work inside constraints
        ↓
RealityTrace
binds expectation to observation
        ↓
settlement evidence
        ↓
GodSpeed-Agent
updates developmental memory and capability
```

For software work, SWE_SEED can define the lighter work-and-proof contract that Gauntlet executes:

```text
SWE_SEED
route + context + artifact + proof contract
        ↓
SEA-Forge
authority
        ↓
Gauntlet
long-horizon execution
```

Those components can be used independently.

The boundaries are useful because they prevent one system from quietly becoming author, executor, observer, verifier, authority, and capability judge at the same time.

---

## When Gauntlet is probably worth trying

Gauntlet starts to make sense when you recognize one of these patterns:

* agent tasks routinely run long enough that you stop watching them
* a task needs several agents or tools to cooperate
* agents frequently retry or rewrite earlier work
* you need work to resume after interruption
* "done" still requires a person to reconstruct what actually happened
* requirements tend to drift during difficult implementation
* one agent both performs and effectively reviews the work
* missing evidence is frequently treated as good enough
* large corrections repeatedly replace smaller diagnoses
* you want to give agents more room without giving them the ability to redefine success

If a task is short, cheap, reversible, and easy to inspect manually, Gauntlet may be unnecessary machinery.

Use the smallest execution structure that gives the work the control it actually needs.

---

## The short version

Freeze what the work must satisfy.

Give the executor room to work.

Observe the resulting state independently.

Materialize the difference.

Verify claims with the evidence they require.

Correct the smallest useful thing.

Preserve what the failure taught.

Resume from records instead of memory.

Stop when the work settles or when no affordable reliable move remains.

Keep one successful run from masquerading as capability.

That is Gauntlet.
