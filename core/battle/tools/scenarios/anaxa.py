"""Build the public, explicitly approximate Anaxa S2 vertical scenario.

Run this file with an output path. Uses only the standard library and public
reference algorithms; no private catalog or raw-config interpreter is loaded.
See docs/modules/battle-simulation/anaxa-scenario.md for usage and the scenario rules.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ELEMENTS = ("physical", "fire", "ice", "thunder", "wind", "quantum", "imaginary")
HERO = "anaxa"


def constant(value: float) -> dict:
    """Create a finite literal expression consumed by the Rust validator."""
    return {"op": "constant", "value": value}


def variable(key: str) -> dict:
    """Read an explicitly initialized persistent actor variable."""
    return {"op": "variable", "key": key}


def binary(op: str, left: dict, right: dict) -> dict:
    """Construct an arithmetic expression without evaluating it in Python."""
    return {"op": op, "left": left, "right": right}


def assign(key: str, value: dict) -> dict:
    """Set an actor variable at runtime."""
    return {"type": "set_dynamic_value", "key": key, "value": value, "target": "self_unit"}


def conditional(condition: dict, effects: list[dict]) -> dict:
    """Construct a conditional with an explicitly empty false branch."""
    return {"type": "conditional", "condition": condition, "then_effects": effects, "else_effects": []}


def has_disclosure(target: str) -> dict:
    """Query a living target's provisional disclosure modifier."""
    return {"type": "has_modifier", "id": "sublimation_disclosure", "target": target}


def modifier(identifier: str, target: str = "self_unit") -> dict:
    """Apply one stack using an explicitly defined modifier."""
    return {"type": "add_modifier", "id": identifier, "stacks": constant(1), "target": target}


def hit(ratio: float, target: str, element: str = "wind", pursued: bool = False) -> dict:
    """Describe one ATK-scaled hit using the engine's existing damage formula."""
    return {"type": "elemental_damage", "scale": "attack", "ratio": constant(ratio),
            "flat": constant(0), "element": {"selection": "random_all"} if pursued else
            {"selection": "fixed", "element": element}, "attack_type": "pursued" if pursued else "direct",
            "can_crit": True, "target": target}


def trigger(event: str, effects: list[dict], action: str | None = None) -> dict:
    """Install an unlimited, actor-filtered trigger (global battle start excepted)."""
    return {"owner": HERO, "event": event, "subject": None if event == "battle_start" else HERO,
            "action": action, "uses": 0, "effects": effects, "expires_on": None, "duration": 0}


def stats(hp: float, attack: float, speed: float) -> dict:
    """Return declared development effective panels, not a player's build."""
    return {"level": 80, "max_hp": hp, "attack": attack, "defense": 0, "speed": speed,
            "taunt": 1, "crit_rate": 0.05, "crit_damage": 0.5, "damage_bonus": 0,
            "defense_reduction": 0, "defense_ignore": 0, "resistance": 0, "resistance_penetration": 0,
            "vulnerability": 0, "damage_reduction": 0, "independent_multiplier": 1,
            "toughness_broken": False, "healing_bonus": 0, "incoming_healing_bonus": 0, "shield_bonus": 0}


def action(identifier: str, kind: str, effects: list[dict], *, cost: int = 0,
           gain: int = 0, energy: float = 0, energy_cost: float = 0) -> dict:
    """Describe an actual action; no scripted three-action carrier is used."""
    return {"id": identifier, "kind": kind, "sp_cost": cost, "sp_gain": gain,
            "energy_cost": energy_cost, "energy_gain": energy, "effects": effects}


def build() -> dict:
    """Return a complete reproducible development scenario with explicit policies.

    Returns:
        JSON-compatible validated-engine input; caller chooses where to save it.
    """
    finish = {"type": "attack_finish"}
    scoped_count = {"op": "scoped_variable", "scope": "ability", "key": "bounce_count"}
    skill = [{"type": "with_dynamic_scope", "scope": "ability", "effects": [
        {"type": "set_scoped_dynamic_value", "scope": "ability", "key": "bounce_count", "value": constant(0)},
        hit(0.7, "ability_target"),
        {"type": "repeat", "count": constant(4), "effects": [
            {"type": "set_scoped_dynamic_value", "scope": "ability", "key": "bounce_count",
             "value": binary("add", scoped_count, constant(1))},
            hit(0.7, "enemy_random_uniform")]}, finish]}]
    hero = {"id": HERO, "stats": stats(6000, 600, 120), "initial_hp": 6000,
            "initial_energy": 0, "max_energy": 140, "revival": None, "actions": [
                action("140503_ultimate", "ultimate", [modifier("sublimation_disclosure", "all_enemies"),
                       modifier("disclosure_damage"), hit(1.6, "all_enemies"), finish], energy_cost=140, energy=5),
                action("140502_skill", "skill", skill, cost=1, energy=30),
                action("140501_basic", "basic", [hit(1.0, "ability_target"), finish], gain=1, energy=30),
                action("anaxa_bonus_skill_approx", "follow_up", [hit(0.7, "ability_target"), finish])
            ]}
    enemies = [{"id": f"enemy_{i}", "stats": stats(2500, 30, 80), "initial_hp": 2500,
                "initial_energy": 0, "max_energy": 100, "revival": None,
                "actions": [action("Monster_AML_Minion01_00_Skill01", "basic", [hit(2.5, "ability_target", "quantum"),
                             {"type": "energy", "amount": 10, "target": "ability_target"}])]}
               for i in range(1, 7)]
    definitions = {
        "s2_crit": {"properties": [{"property": "crit_damage", "value": constant(0.6)}], "lifetime": None},
        "s2_team_damage": {"properties": [{"property": "damage_bonus", "value": constant(0.3)}], "lifetime": None},
        "sublimation_disclosure": {"properties": [], "lifetime": {"event": "turn_start", "unit": "owner", "ticks": 1}},
        "disclosure_damage": {"properties": [{"property": "damage_bonus", "value": constant(0.3)}],
                              "lifetime": {"event": "action_end", "unit": "owner", "ticks": 1}},
    }
    for definition in definitions.values():
        definition.update(stack_mode="replace_by_source", max_stacks=1)
    increment = [assign("Skill_Count", binary("add", variable("Skill_Count"), constant(1))),
                 conditional({"type": "compare", "left": variable("Skill_Count"), "comparison": "greater_equal", "right": constant(2)}, [
                     assign("Skill_Count", binary("subtract", variable("Skill_Count"), constant(2))),
                     assign("GridFight_S2_Current_MagicLayer", binary("add", variable("GridFight_S2_Current_MagicLayer"), constant(1))),
                     assign("grid_fight_custom:108", binary("add", variable("grid_fight_custom:108"), constant(1)))])]
    pursuit = [assign("Purse_Attack_Count", {"op": "floor", "value": binary("divide", variable("GridFight_S2_Current_MagicLayer"), constant(5))}),
               {"type": "repeat", "count": variable("Purse_Attack_Count"), "effects": [
                   {"type": "retarget_attack_random", "effects": [hit(0.7, "context_target", pursued=True)], "empty_effects": []}]}]
    hooks = [trigger("battle_start", [modifier("s2_crit"), modifier("s2_team_damage")]),
             trigger("turn_start", [conditional({"type": "not", "condition": has_disclosure("all_enemies")},
                                                  [{"type": "energy", "amount": 30, "target": "self_unit"}])])]
    for identifier in ("140502_skill", "anaxa_bonus_skill_approx"):
        hooks += [trigger("before_skill_use", increment, identifier), trigger("after_attack", pursuit, identifier)]
    for identifier in ("140501_basic", "140502_skill"):
        hooks.append(trigger("before_skill_use", [conditional(has_disclosure("ability_target"), [
            modifier("disclosure_damage"), {"type": "request", "kind": "follow_up",
            "action": "anaxa_bonus_skill_approx", "target": "self_unit"}])], identifier))
    hooks.append(trigger("before_skill_use", [conditional(has_disclosure("ability_target"), [modifier("disclosure_damage")])], "anaxa_bonus_skill_approx"))
    return {
        "schema_version": 1, "ruleset": "battle_v2",
        "provenance": {"game_version": "4.7-reference/development-input",
                       "data_snapshot": "anaxa_s2_explicit_queue_v1",
                       "description": "Public Anaxa approximate vertical scenario; developer panels, finite queue, no private data"},
        "unsupported_mechanics": [], "max_av": 2400, "max_actions": 10000,
        "ally_skill_points": {"initial": 3, "maximum": 5}, "enemy_skill_points": {"initial": 0, "maximum": 5},
        "allies": [hero], "waves": [enemies],
        "mechanics": {
            "queue_capacity": 2, "ally_damage_factor": 0.3, "cycles": [2400],
            "unit_before_cycle": False, "triggers": hooks, "modifier_definitions": definitions,
            "dynamic_values": {HERO: {"Skill_Count": 0, "GridFight_S2_Current_MagicLayer": 0,
                                      "grid_fight_custom:108": 0, "Purse_Attack_Count": 0}},
            "natural_weaknesses": {u["id"]: ["physical", "wind"] for u in [hero, *enemies]},
            "modifier_weaknesses": {"sublimation_disclosure": list(ELEMENTS)},
            "element_profiles": {u["id"]: {"damage_bonus": dict.fromkeys(ELEMENTS, 0),
                                            "resistance": dict.fromkeys(ELEMENTS, 0)} for u in [hero, *enemies]},
            "settlement": {"name": "weighted_kills_flat_penalty_v1", "enemy_weights": {u["id"]: 1 for u in enemies},
                           "minimum_progress": 0.5, "hp_before": 60, "hp_cap": 100, "failure_loss": 20, "recovery": 3},
            "assumptions": [
                "configured: anaxa_s2_explicit_queue_v1; role 140502; basic level 6, skill/ultimate level 10; no equipment, active traits, eidolons or technique; explicit developer effective panels (not catalog/player measurements).",
                "provisional: anaxa_nous_approx_v1; HSR_Nous f8935ddb407b297732fbd3c624d55897e3e648b9 MIT template 1405; S2 callbacks from Dimbreath dad9155eb00fdeda31fb8456507c53590d49071f. These references do not establish native accuracy.",
                "configured: first_affordable_v1; ordered ultimate > skill > basic; primary lowest HP fraction then entry index; seeded uniform living-enemy bounces; ultimate inserts at existing legal action boundaries.",
                "provisional: skill has one selected and four uniform bounce hits, 30 total energy; basic energy 30; ultimate cost 140/gain 5; no-disclosure TurnStart grants 30, including inserted ultimate TurnStart. No per-hit energy interpretation of unknown raw opcodes.",
                "provisional: ultimate gives seven weaknesses/disclosure until target TurnStart; basic/skill on disclosed entry target queues one 0.7 ATK bonus segment, no SP/energy, classified FollowUp for scheduling but counted as a skill by the explicit S2 hooks; no recursive bonus.",
                "provisional: disclosed entry-target damage bonus 0.3 applies to the entire action, including bounces/pursuits; ultimate applies it before AoE. Bonus target is reselected at entry after replacement. Native target-specific semantics not reproduced.",
                "omitted: per-hit random weakness implant, hard control/control resistance, per-weakness defense ignore, enemy-count skill bonus, static trace-2 bonuses, toughness damage/break recovery, other hit-taken energy and native AI; may change damage/order/outcome.",
                "configured: six development instances use the public Monster_AML_Minion01_00 Skill01 single Quantum hit (2.5 ATK), with explicit target energy 10; other enemy skills/passives are excluded. All panels/HP/queue/capacity are explicit developer inputs; no native encounter generation, summon, escape or phase mechanics.",
                "provisional: finite_queue_after_action_v1; ascending empty slot refill after complete actions, before next inserts; new enemy begins at full action distance; immediate revival keeps slot; queue exhausted AND board empty required for clear. Progress denominator fixed; same final-dead instance counted once; reaching threshold does not stop battle.",
                "provisional: weighted_kills_flat_penalty_v1; threshold failure incurs configured flat loss; floor loss at zero then apply configured recovery up to cap, no rounding/cap changes (recovery can restore zero after loss). Battle outcome, minimum progress and run survival are separate. Cancellation/ActionLimit never settle HP.",
                "configured: existing Sakura cycle/action ordering and 0.3 ally damage factor; no additional inferred difficulty reduction. Timer ties use unit_before_cycle=false; completed action elimination precedes expiry. Runtime error is an error, never a business defeat."
            ]
        }
    }


def main() -> None:
    """Write the explicitly selected scene to the required output path."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(build(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
