"""Counterexamples for strict raw Config lowering; fixtures are artificial."""
import unittest
import json
from pathlib import Path
import tempfile
from tools.raw_config.actions import Compiler, Unsupported


class ConfigTests(unittest.TestCase):
    """Check branch selection, required bindings, and unknown operations."""

    def setUp(self):
        """Create a compiler without bundled copyrighted game data."""
        self.compiler = Compiler([], {'7': 0.2}, ['PointB3'], 10.0)

    def test_inverse_predicate(self):
        """Inverse must invert the selected regular-skill branch."""
        self.assertFalse(self.compiler.predicate({'$type':'RPG.GameCore.BySkillPointActivated','PointTriggerKey':'PointB3','Inverse':True}))

    def test_missing_dynamic_is_not_zero(self):
        """An unresolved runtime binding must stop compilation."""
        with self.assertRaisesRegex(Unsupported, 'unresolved dynamic hash 9'):
            self.compiler.number({'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQAR','FixedValues':[],'DynamicHashes':[9]}})

    def test_observed_negation_opcode(self):
        """The sourced delay expression uses a load followed by negation."""
        self.assertEqual(self.compiler.number({'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQAOEQ==','FixedValues':[],'DynamicHashes':[7]}}),-0.2)

    def test_unknown_task_is_located(self):
        """Never drop an unknown logic node."""
        with self.assertRaisesRegex(Unsupported, 'probe/0: task UnknownDamage'):
            self.compiler.tasks([{'$type':'RPG.GameCore.UnknownDamage'}],'probe')

    def test_hit_energy_is_lowered_with_ratio(self):
        """Hit energy is numerical behavior, separate from split damage."""
        effects=self.compiler.tasks([{'$type':'RPG.GameCore.DamageByAttackProperty','TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'AbilityTargetEntity'},'AttackProperty':{'$type':'RPG.GameCore.AttackData','DamageType':{'DamageType':'Quantum'},'DamagePercentage':{'Value':2.5},'SPHitRatio':{'Value':0.5}}}],'probe')
        self.assertEqual(effects[-1],{'type':'energy','amount':5.0,'target':'ability_target'})

    def test_explicit_unimplemented_last_kill_flag_rejects(self):
        """Do not silently accept false or nonboolean last-kill behavior."""
        for value in [False, 0, 1, None]:
            with self.subTest(value=value), self.assertRaisesRegex(Unsupported, 'unimplemented CanTriggerLastKill value'):
                self.compiler.tasks([{'$type':'RPG.GameCore.DamageByAttackProperty','CanTriggerLastKill':value}], 'probe')

    def test_nonempty_unknown_ability_fields_reject_before_lowering(self):
        """An unhandled callback cannot disappear behind a compiled OnStart."""
        damage = {'$type':'RPG.GameCore.DamageByAttackProperty',
                  'TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'AbilityTargetEntity'},
                  'AttackProperty':{'$type':'RPG.GameCore.AttackData','DamageType':{'DamageType':'Quantum'},
                                    'DamagePercentage':{'Value':1}}}
        for field, value in [('OnEnd', [{'$type':'RPG.GameCore.ModifySPNew'}]),
                             ('OnRemove', [{'$type':'RPG.GameCore.ModifySPNew'}]),
                             ('UnknownNumericValue', 0)]:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'ability.json'
                path.write_text(json.dumps({'AbilityList':[{'Name':'entry',
                    'TargetInfo':{'TargetType':'SkillTargetEntityList'}, 'OnStart':[damage], field:value}]}))
                compiler = Compiler([path], {}, [], 10.0)
                with self.assertRaisesRegex(Unsupported, f'#/AbilityList/0/{field}: unsupported ability field'):
                    compiler.ability('entry')
                self.assertEqual(compiler.stack, [])
                self.assertEqual(compiler.evidence[-1]['handling'], 'rejected')

    def test_empty_unknown_callback_has_no_logic_to_execute(self):
        """An explicitly empty unsupported callback is recorded as empty."""
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'ability.json'
            path.write_text(json.dumps({'AbilityList':[{'Name':'entry','OnStart':[],'OnEnd':[]}]}))
            compiler = Compiler([path], {}, [], 10.0)
            self.assertEqual(compiler.ability('entry'), [])
            self.assertEqual(compiler.evidence[-1]['handling'], 'empty_unsupported_field')

    def test_unhandled_target_metadata_rejects(self):
        """Ability target metadata must match the supported skill-target contract."""
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'ability.json'
            path.write_text(json.dumps({'AbilityList':[{'Name':'entry',
                'TargetInfo':{'TargetType':'AllEntities'},'OnStart':[]}]}))
            compiler = Compiler([path], {}, [], 10.0)
            with self.assertRaisesRegex(Unsupported, '#/AbilityList/0/TargetInfo: unsupported ability target'):
                compiler.ability('entry')


if __name__=='__main__':
    unittest.main()
