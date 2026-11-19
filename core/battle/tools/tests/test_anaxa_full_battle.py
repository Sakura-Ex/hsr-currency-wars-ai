"""Check the public builder contract and its checked-in, deterministic artifact."""
import json
import unittest
from pathlib import Path

from tools.scenarios.anaxa import build


class PublicScenarioTests(unittest.TestCase):
    """Keep the runnable fixture reproducible without local/private dependencies."""

    def test_checked_in_scene_matches_builder(self) -> None:
        """Reconstruction must match the complete checked-in scene exactly."""
        scene = Path(__file__).resolve().parents[2] / "examples" / "anaxa-s2-full-battle.json"
        self.assertEqual(json.loads(scene.read_text(encoding="utf-8")), build())

    def test_initial_state_has_no_precharged_passive_or_energy(self) -> None:
        """Required triggers must arise during runtime, never from primed resources."""
        scene = build()
        self.assertEqual(scene["allies"][0]["initial_energy"], 0)
        self.assertEqual(scene["mechanics"]["dynamic_values"]["anaxa"]["GridFight_S2_Current_MagicLayer"], 0)
        self.assertEqual(len(scene["waves"][0]), 6)
        self.assertEqual(scene["mechanics"]["queue_capacity"], 2)
        self.assertTrue(any("omitted:" in line for line in scene["mechanics"]["assumptions"]))


if __name__ == "__main__":
    unittest.main()
