# Rust basic battle core

This library executes explicit, already resolved battle scenarios. The `basic_v1`
profile supports speed/action-value scheduling, teams and enemy waves, skill
points and energy, automatic action priorities, direct damage and critical hits,
weighted random targets, healing, shields, action advance/delay, and optional
explicit revival. It returns an event trace or a reproducible Monte Carlo ledger.

The profile is a bounded simulation contract, not a claim of complete Currency
Wars 4.7 character coverage. It does not convert accounts or infer missing kits,
break mechanics, buffs, DoTs, summons, follow-up triggers, background units, or
shared run-HP settlement. A caller must declare known unsupported mechanisms;
validation rejects them. Unknown input fields and effects are also rejected.
Existing private-data adapters are not bundled in this public core.

## Architecture and execution contract

`model::validate` checks the complete input once and produces an immutable
`ValidatedScenario` with a SHA-256 identity. `engine` owns single-battle state;
`math` owns pure damage/heal/shield formulae; `batch` owns independent trial seeds,
outcome ledgers and frequency estimates. CLI and future Tauri commands use the
same Rust functions. There is no second set of frontend combat formulae.

The following semantics define `basic_v1` and are exposed rather than inferred:

- Initial normal actions occur at `10000 / speed`. Ties follow stable input order
  (allies, then the active enemy wave). Time never moves backwards.
- Ultimate actions with sufficient resources are checked at settled action
  boundaries, in the same stable unit order. They do not consume a normal turn.
  Other actions use each unit's declared priority, choosing the first affordable
  non-ultimate action. A free fallback is required. Strategy never sees the RNG.
- Costs are validated and paid before effects. Effects execute in declared order.
  After effects, the actor receives its declared energy/SP gain if still alive.
  Energy and skill points are capped. Enemy skill points are separate and persist
  across waves. All enemies in a new wave use their declared initial energy.
- Random-target damage retargets each hit from living opponents using positive
  taunt weights. Each critical-capable hit samples its own critical outcome.
  Target and critical RNG streams are separated using SHA-256 and ChaCha8.
- Shields absorb damage before HP. Shield grants add up to their declared cap and
  refresh their duration. Duration decreases at the start of the recipient's next
  scheduled turn; a duration of one protects until that turn starts. Ultimates do
  not tick shield duration. This is an explicit primitive, not a universal kit rule.
- Action advance shifts the remaining scheduled action by `fraction * 10000/speed`
  and clamps to the present. Negative fractions delay. A normal actor's next action
  is scheduled before its effects, so self-advance affects the next action.
- Death is permanent unless an explicit revival specification has charges left.
  Such revival occurs immediately at the declared HP fraction, clears shields,
  preserves energy, schedules a new full interval, and adds the declared AV penalty
  to a separate budget ledger. No client `0.25` field is interpreted as AV.
- Waves share natural elapsed AV, accumulated revival penalties, actions and ally
  state. New enemies are scheduled relative to the current time. No wave gets a
  fresh budget. An event exactly at the AV deadline is not executed.
- After an action's effects settle, terminal elimination is checked before the
  next budget check. A completed final-wave elimination is victory. Budget
  exhaustion is defeat under this declared deadline profile; a safety action
  limit or cancellation is incomplete and excluded from outcome frequencies.

All quantities are finite; malformed values, unsupported effects, illegal costs,
ambiguous identities and arithmetic failure are errors. The core never substitutes
zero for an unknown game value. `run_hp_loss` is unknown because node combat and
shared run settlement have different owners.

## Running

From the repository workspace, use Cargo with the checked-in lockfile:

```text
cargo test --locked -p hcwa-battle-core
cargo run --locked -p hcwa-battle-core --bin hcwa-battle -- validate SCENARIO.json
cargo run --locked -p hcwa-battle-core --bin hcwa-battle -- simulate SCENARIO.json SEED
cargo run --release --locked -p hcwa-battle-core --bin hcwa-battle -- batch SCENARIO.json TRIALS SEED
```

The CLI writes JSON to stdout and errors to stderr with exit code 2. Redirect
results to a local output path; do not commit generated ledgers or player inputs.
Examples use artificial units and are marked as synthetic. Their frequencies
test the engine, not a named character's real-game win probability.

Seeds remain `u64` in Rust and are decimal strings in JSON. TypeScript callers
must preserve the string rather than round it through a JavaScript Number.
The library's cancellable entry points check between actions, including zero-AV
ultimate chains. Trace output is capped at 100,000 events; `trace_truncated`
reports that cap without changing the simulated state or batch counts.

## Reuse and validation

The pure math implementation ports the MIT-licensed HSR_Nous rulebook at commit
`f8935ddb407b297732fbd3c624d55897e3e648b9`. Attribution and its license are retained
in `math.rs` and `THIRD_PARTY_NOTICES.md`. The profile does not adopt that engine's
known budget or checkpoint defects. Serde, SHA-2 and rand_chacha provide parsing,
fingerprints and reproducible random streams; versions are fixed by Cargo.lock.

Validation separates hand-computed arithmetic and state-transition tests,
upstream formula parity, deterministic seed replay, statistical smoke checks, and
actual game validation. Only the first four are available for this initial core.
Private current-version records may be used locally to audit adapter coverage;
they are not embedded in source or represented as complete playable kits.
