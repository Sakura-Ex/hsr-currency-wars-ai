"""Regression examples for review F1: fields cannot disappear behind known types."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

from tools.raw_config.actions import Compiler, Unsupported, PRESENTATION, WAITS, MARKERS
from tools.raw_config.expression import compile_expression


def target(alias: str) -> dict:
    """Create an explicit synthetic target alias."""
    return {'$type': 'RPG.GameCore.TargetAlias', 'Alias': alias}


class FieldConsumptionTests(unittest.TestCase):
    """Exercise the whole supported task family, plus nested container boundaries."""

    def setUp(self) -> None:
        """Use explicit finite bindings without game or private fixture dependencies."""
        self.compiler = Compiler([], {'7': 0.2}, [], 10.0)

    def reject(self, node: dict, pointer: str) -> None:
        """Require rejection to identify the particular unconsumed field."""
        with self.assertRaises(Unsupported) as caught:
            self.compiler.tasks([node], 'probe')
        self.assertIn(pointer, str(caught.exception))
        self.assertEqual(self.compiler.evidence[-1]['handling'], 'rejected')

    def test_original_wait_probe_and_nested_payload_with_valid_time(self) -> None:
        """Both the exact review input and a valid WaitTime with hidden tasks reject."""
        node = {'$type': 'RPG.GameCore.WaitSecond', 'Seconds': {'Value': 0.1},
                'TaskList': [{'$type': 'RPG.GameCore.UnknownNumericTask', 'Value': 10}]}
        self.reject(node, 'probe/0/Seconds')
        node['WaitTime'] = node.pop('Seconds')
        self.reject(node, 'probe/0/TaskList')

    def test_original_delay_target_probe_and_caster_positive(self) -> None:
        """An explicit enemy target cannot silently become a self advance."""
        node = {'$type': 'RPG.GameCore.ModifyCurrentSkillDelayCost',
                'NormalizedValue': {'Value': -0.2}, 'TargetType': target('AbilityTargetEntity')}
        self.reject(node, 'probe/0/TargetType/Alias')
        node['TargetType'] = target('Caster')
        self.assertEqual(self.compiler.tasks([node], 'probe'), [
            {'type': 'advance_next', 'fraction': 0.2, 'target': 'self_unit'}])

    def test_original_predicate_probe_outer_and_inner_fields(self) -> None:
        """Check both wrappers independently rather than masking the inner failure."""
        node = {'$type': 'RPG.GameCore.PredicateTaskList', 'UnknownNumericField': 123,
                'Predicate': {'$type': 'RPG.GameCore.ByIsInsertAction', 'UnknownNumericField': 123},
                'SuccessTaskList': [], 'FailedTaskList': []}
        self.reject(node, 'probe/0/UnknownNumericField')
        del node['UnknownNumericField']
        self.reject(node, 'probe/0/Predicate/UnknownNumericField')

    def test_every_supported_task_rejects_extra_fields(self) -> None:
        """Field closure applies across tasks, not only to the three reported cases."""
        kinds = PRESENTATION | WAITS | MARKERS | {
            'TriggerAbility', 'PredicateTaskList', 'DamageByAttackProperty',
            'ModifyCurrentSkillDelayCost', 'AddModifier'}
        for kind in sorted(kinds):
            with self.subTest(kind=kind):
                self.reject({'$type': 'RPG.GameCore.' + kind, 'Unconsumed': []}, 'probe/0/Unconsumed')

    def test_presentation_nested_shapes_reject_logic(self) -> None:
        """Known display fields are not opaque dictionaries containing task lists."""
        cases = [
            ({'$type': 'RPG.GameCore.VCameraConfigChange', 'CameraConfig': {'NormalConfig': {'TaskList': []}}},
             'CameraConfig/NormalConfig/TaskList'),
            ({'$type': 'RPG.GameCore.TriggerAnimStateWithMove', 'MovingRangeList': [{'TargetType': target('Caster'), 'TaskList': []}]},
             'MovingRangeList/0/TaskList'),
            ({'$type': 'RPG.GameCore.WaitSecond', 'WaitTime': {'Value': 0.1, 'TaskList': []}},
             'WaitTime/TaskList'),
            ({'$type': 'RPG.GameCore.LookAt', 'TargetType': {'TaskList': []}}, 'TargetType'),
        ]
        for node, suffix in cases:
            with self.subTest(suffix=suffix):
                self.reject(node, 'probe/0/' + suffix)

    def test_supported_presentation_fields_remain_valid(self) -> None:
        """Inspected animation, movement and camera values keep their zero-AV role."""
        nodes = [
            {'$type': 'RPG.GameCore.WaitSecond', 'WaitTime': {'IsDynamic': False, 'FixedValue': {'Value': 0.1}}},
            {'$type': 'RPG.GameCore.VCameraConfigChange', 'CameraConfig': {
                'NormalConfig': {'CameraState': 'CameraMotion', 'ResetToDefault': True, 'AnchorOffset': {'X': 1}},
                'BlendConfig': {'BlendType': 'Custom', 'BlendTime': 0.2, 'CustomCurveName': 'example'},
                'OverrideShakeConfigV2': {'ShakeDir': {'X': 0, 'Y': 1, 'Z': 0}, 'ShakeScale': 0.1, 'ShakeTime': 0.2}}},
            {'$type': 'RPG.GameCore.TriggerAnimStateWithMove', 'TargetType': target('Caster'), 'AnimStateName': 'Skill01',
             'MovingRangeList': [{'TargetType': target('AbilityTargetEntity'), 'OffsetTargetDistance': {'Value': 1.3},
                                  'NormalizedTimeStart': {'Value': 0}, 'NormalizedTimeEnd': {'Value': 1}}]},
            {'$type': 'RPG.GameCore.DamagePerformFinish'},
        ]
        self.assertEqual(self.compiler.tasks(nodes, 'probe'), [])
        self.assertTrue(all(row['handling'] in {'presentation_only', 'ordered_marker_zero_av'} for row in self.compiler.evidence))

    def test_numeric_wrappers_and_unused_operands_are_closed(self) -> None:
        """Legacy scalar and shared runtime expression compilers reject the same payloads."""
        cases = [
            ({'Value': 2, 'TaskList': []}, '/TaskList'),
            ({'IsDynamic': False, 'FixedValue': {'Value': 2, 'Other': 0}}, '/FixedValue/Other'),
            ({'IsDynamic': True, 'PostfixExpr': {'OpCodes': 'AQAR', 'FixedValues': [], 'DynamicHashes': [7], 'Other': 0}}, '/PostfixExpr/Other'),
            ({'IsDynamic': True, 'PostfixExpr': {'OpCodes': 'AQAR', 'FixedValues': [{'Value': 1, 'Other': 0}], 'DynamicHashes': [7]}}, '/PostfixExpr/FixedValues/0/Other'),
        ]
        for node, suffix in cases:
            with self.subTest(suffix=suffix):
                with self.assertRaises(Unsupported) as old:
                    self.compiler.number(node, 'value')
                self.assertIn('value' + suffix, str(old.exception))
                with self.assertRaises(ValueError) as runtime:
                    compile_expression(node, {'7': 0.2}, 'value')
                self.assertIn('value' + suffix, str(runtime.exception))

    def test_reference_target_and_name_wrapper_are_consumed(self) -> None:
        """References retain explicit caster context and cannot hide nested logic."""
        self.compiler.abilities['child'] = ({'Name': 'child', 'OnStart': []}, 'source#/AbilityList/0')
        node = {'$type': 'RPG.GameCore.TriggerAbility', 'TargetType': target('Caster'),
                'AbilityName': {'Value': 'child'}, 'IsSkillPerform': True}
        self.assertEqual(self.compiler.tasks([node], 'probe'), [])
        bad = copy.deepcopy(node); bad['AbilityName']['Unknown'] = 1
        self.reject(bad, 'probe/0/AbilityName/Unknown')
        bad = copy.deepcopy(node); bad['TargetType'] = target('AbilityTargetEntity')
        self.reject(bad, 'probe/0/TargetType/Alias')
        bad = copy.deepcopy(node); bad['IsSkillPerform'] = False
        self.reject(bad, 'probe/0/IsSkillPerform')

    def test_modifier_payload_and_callbacks_are_checked(self) -> None:
        """Pure preview bookkeeping passes; hidden state callbacks/values do not."""
        definition = {'ModifierAffectedPreshowConfig': {
            'SkillTypes': ['Normal'], 'TargetType': target('ModifierOwnerEntity'),
            'ActionDelayPreshowConfig': {'AddDelayRatio': {'Value': -0.2}}},
            '_CallbackList': [{'Event': 'OnActionEnd', 'CallbackConfig': [{'$type': 'RPG.GameCore.RemoveSelfModifier'}]}],
            'Stacking': 'Multiple'}
        task = {'$type': 'RPG.GameCore.AddModifier', 'TargetType': target('Caster'), 'ModifierName': {'Value': 'preview'}}
        cases = [('valid', None), ('callback_extra', '/_CallbackList/0/CallbackConfig/0/Unknown'),
                 ('preview_nested', '/ModifierAffectedPreshowConfig/TaskList'), ('dynamic_extra', '/OnStart/0/DynamicValues/Unknown')]
        for case, expected in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as folder:
                modifier = copy.deepcopy(definition); node = copy.deepcopy(task)
                if case == 'callback_extra': modifier['_CallbackList'][0]['CallbackConfig'][0]['Unknown'] = 1
                if case == 'preview_nested': modifier['ModifierAffectedPreshowConfig']['TaskList'] = []
                if case == 'dynamic_extra': node['DynamicValues'] = {'Unknown': {'Value': 3}}
                path = Path(folder) / 'source.json'
                path.write_text(json.dumps({'AbilityList': [{'Name': 'entry', 'Modifiers': {'preview': modifier}, 'OnStart': [node]}]}))
                compiler = Compiler([path], {}, [], 0)
                if expected is None:
                    self.assertEqual(compiler.ability('entry'), [])
                else:
                    with self.assertRaises(Unsupported) as caught: compiler.ability('entry')
                    self.assertIn(expected, str(caught.exception))

    def test_unselected_branch_is_reported_and_malformed_list_rejects(self) -> None:
        """Known regular context excludes insert-only logic without claiming coverage."""
        node = {'$type': 'RPG.GameCore.PredicateTaskList', 'Predicate': {'$type': 'RPG.GameCore.ByIsInsertAction'},
                'SuccessTaskList': [{'$type': 'RPG.GameCore.UnknownInsertOnlyTask'}],
                'FailedTaskList': [{'$type': 'RPG.GameCore.ModifyCurrentSkillDelayCost', 'NormalizedValue': {'Value': -0.2}}]}
        self.assertEqual(self.compiler.tasks([node], 'probe')[0]['fraction'], 0.2)
        excluded = [e for e in self.compiler.evidence if e['handling'] == 'not_selected_declared_regular_context']
        self.assertEqual(excluded[0]['pointer'], 'probe/0/SuccessTaskList')
        self.assertEqual(excluded[0]['task_count'], 1)
        node['SuccessTaskList'] = {'TaskList': []}
        self.reject(node, 'probe/0/SuccessTaskList')


if __name__ == '__main__':
    unittest.main()
