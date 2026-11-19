"""Artificial control-flow counterexamples; no raw game data is bundled."""
import unittest
from tools.raw_config.actions import Compiler, Unsupported
from tools.raw_config.expression import stable_hash
from tools.raw_config.runtime import RuntimeLowering


class RuntimeTests(unittest.TestCase):
    """Exercise live state lowering and rejection of unimplemented branches."""
    def setUp(self):
        """Prepare artificial immutable parameters and one declared local key."""
        self.compiler = Compiler([], {'7': 2.0}, [], 0.0)
        self.lowering = RuntimeLowering(self.compiler, {stable_hash('count'): 'modifier:count'})

    def test_live_comparison_is_not_folded_with_initial_state(self):
        """Keep a live read on the left and a resolved parameter on the right."""
        node = {'$type':'RPG.GameCore.ByCompareDynamicValue',
                'DynamicKey':{'Value':'count'}, 'CompareType':'GreaterEqual',
                'CompareValue':{'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQAR','FixedValues':[],'DynamicHashes':[7]}}}
        result = self.lowering.condition(node)
        self.assertEqual(result['left'], {'op':'variable','key':'modifier:count'})
        self.assertEqual(result['right'], {'op':'constant','value':2.0})

    def test_unknown_task_in_either_branch_rejects_at_pointer(self):
        """Reject an unsupported operation even in an unselected branch."""
        task = {'$type':'RPG.GameCore.PredicateTaskList',
                'Predicate':{'$type':'RPG.GameCore.ByCurrentSkillType','SkillType':'Skill'},
                'SuccessTaskList':[], 'FailedTaskList':[{'$type':'RPG.GameCore.UnknownNumeric'}]}
        with self.assertRaisesRegex(Unsupported, 'callback/0/FailedTaskList/0: runtime task UnknownNumeric'):
            self.lowering.tasks([task], 'callback')

    def test_storage_scope_and_extra_fields_cannot_be_inferred(self):
        """Reject unknown storage and an unimplemented explicit ContextScope."""
        task = {'$type':'RPG.GameCore.SetDynamicValue','DynamicKey':{'Value':'other'},'Value':{'Value':1.0}}
        with self.assertRaisesRegex(Unsupported, 'undeclared runtime storage'):
            self.lowering.tasks([task], 'callback')
        task['DynamicKey']['Value'] = 'count'
        task['ContextScope'] = 'ContextAbility'
        with self.assertRaisesRegex(Unsupported, 'ContextScope ContextAbility does not match'):
            self.lowering.tasks([task], 'callback')

    def test_persistent_custom_property_requires_runtime_value(self):
        """Preserve the current persistent value as a required runtime read."""
        task = {'$type':'RPG.GameCore.ModifyGridFightCustomProperty',
                'CustomType':{'EnumIndex':2,'Value':108},'ModifyValue':{'Value':1.0}}
        effect = self.lowering.tasks([task], 'callback')[0]
        self.assertEqual(effect['key'], 'grid_fight_custom:108')
        self.assertEqual(effect['value']['left'], {'op':'variable','key':'grid_fight_custom:108'})

    def test_same_hash_can_bind_to_ability_or_modifier_without_aliasing(self):
        """The raw name is not itself a storage location or lifecycle."""
        key = stable_hash('_count')
        ability = RuntimeLowering(self.compiler, {key: {'scope':'ability','key':'_count'}})
        modifier = RuntimeLowering(self.compiler, {key: 'modifier:skilltree02:_count'})
        raw = {'IsDynamic':True, 'PostfixExpr':{'OpCodes':'AQAR','FixedValues':[],'DynamicHashes':[int(key)]}}
        self.assertEqual(ability.expression(raw), {'op':'scoped_variable','scope':'ability','key':'_count'})
        self.assertEqual(modifier.expression(raw), {'op':'variable','key':'modifier:skilltree02:_count'})
        task = {'$type':'RPG.GameCore.SetDynamicValueByAddValue',
                'ContextScope':'ContextAbility', 'Key':{'Value':'_count'}, 'AddValue':{'Value':1}}
        effect = ability.tasks([task], 'ability')[0]
        self.assertEqual(effect['type'], 'set_scoped_dynamic_value')
        self.assertEqual(effect['value']['left'], ability.expression(raw))
        with self.assertRaisesRegex(Unsupported, 'ContextScope ContextAbility does not match'):
            modifier.tasks([task], 'modifier')

    def test_template_initialization_requires_value_and_matching_scope(self):
        """An omitted native reset cannot silently initialize a scoped zero."""
        lowering = RuntimeLowering(self.compiler, {stable_hash('count'):{'scope':'task_template','key':'count'}})
        node = {'$type':'RPG.GameCore.DefineDynamicValue', 'DynamicKey':{'Value':'count'},
                'ContextScope':'ContextTaskTemplate'}
        with self.assertRaisesRegex(Unsupported, 'lacks explicit ResetValue'):
            lowering.tasks([node], 'template')
        node['ResetValue'] = {'Value':0}
        self.assertEqual(lowering.tasks([node], 'template')[0],
            {'type':'set_scoped_dynamic_value','scope':'task_template','key':'count','value':{'op':'constant','value':0.0}})
        node['ContextScope'] = 'ContextAbility'
        with self.assertRaisesRegex(Unsupported, 'ContextScope ContextAbility does not match'):
            lowering.tasks([node], 'template')
        node['ContextScope'] = None
        with self.assertRaisesRegex(Unsupported, 'unsupported explicit ContextScope'):
            lowering.tasks([node], 'template')

    def test_property_lowering_cannot_skip_lifecycle_callbacks(self):
        """Do not declare a property modifier complete when another callback exists."""
        definition = {'_CallbackList':[{'Event':'OnStack','CallbackConfig':[
            {'$type':'RPG.GameCore.StackProperty','Property':'CriticalDamageBase',
             'TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'ModifierOwnerEntity'},
             'PropertyValue':{'Value':0.6}}]}]}
        result = self.lowering.property_changes(definition, 'modifier')
        self.assertEqual(result, [{'property':'crit_damage','value':{'op':'constant','value':0.6}}])
        definition['_CallbackList'].append({'Event':'OnDestroy','CallbackConfig':[]})
        with self.assertRaisesRegex(Unsupported, 'unsupported property modifier callback OnDestroy'):
            self.lowering.property_changes(definition, 'modifier')

    def test_presentation_template_cannot_hide_numeric_tasks(self):
        """Inspect delayed/conditional branches instead of trusting a template name."""
        name = 'PursuedDamage_PerformanceDelay'
        lowering = RuntimeLowering(self.compiler, {}, {name: {'Name':name,'TaskList':[
            {'$type':'RPG.GameCore.PerformDelayExecute','PerformTaskList':[
                {'$type':'RPG.GameCore.DamageByAttackProperty'}]}]}})
        with self.assertRaisesRegex(Unsupported, 'non-presentation template node DamageByAttackProperty'):
            lowering.presentation_delay_template(name)

    def test_retarget_does_not_expand_pool_or_drop_failure_branch(self):
        """Only sourced one-target selection is accepted; both task lists compile."""
        node = {'$type':'RPG.GameCore.Retarget',
                'TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'AttackTargetList'},
                'ByRandom':True,'MaxNumber':{'Value':1},'TaskList':[],
                'FailedTaskList':[{'$type':'RPG.GameCore.UnknownNumeric'}]}
        with self.assertRaisesRegex(Unsupported, 'FailedTaskList/0: runtime task UnknownNumeric'):
            self.lowering.tasks([node], 'retarget')

    def test_pursued_hit_preserves_kind_and_requires_explicit_last_kill(self):
        """Do not reinterpret a pursued hit as the carrier skill or infer a flag."""
        node = {'$type':'RPG.GameCore.DamageByAttackProperty',
                'TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'ParamEntity'},
                'CanTriggerLastKill':True,
                'AttackProperty':{'$type':'RPG.GameCore.AttackData',
                    'AttackType':'Pursued','DamageType':{'SourceType':'Random'},
                    'DamagePercentage':{'Value':0.7}}}
        result = self.lowering.tasks([node], 'pursued')
        self.assertEqual(result[0]['attack_type'], 'pursued')
        for value in [False, 0, 1, None]:
            node['CanTriggerLastKill'] = value
            with self.subTest(value=value), self.assertRaisesRegex(Unsupported, 'explicit CanTriggerLastKill=true'):
                self.lowering.tasks([node], 'pursued')
        del node['CanTriggerLastKill']
        with self.assertRaisesRegex(Unsupported, 'explicit CanTriggerLastKill=true'):
            self.lowering.tasks([node], 'pursued')


if __name__ == '__main__':
    unittest.main()
