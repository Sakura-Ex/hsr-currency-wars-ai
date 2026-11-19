"""Strict runtime lowering for sourced dynamic counter and control-flow tasks.

This extends the selected-chain compiler; it is not a complete ability importer.
Callers must provide explicit storage keys for runtime variables. Modifier-local,
unit-local and persistent values therefore cannot silently share one namespace.
"""
from .errors import Unsupported
from .expression import compile_expression, stable_hash


class RuntimeLowering:
    """Lower supported callbacks while retaining unresolved mechanism failures."""

    def __init__(self, compiler, runtime_keys: dict[str, str | dict], templates: dict | None = None):
        """Share immutable bindings/evidence and retain explicit storage mappings."""
        self.compiler = compiler
        self.runtime_keys = runtime_keys
        self.templates = templates if templates is not None else {}

    def presentation_delay_template(self, name: str) -> None:
        """Check a supplied timing-only template before excluding animation delay.

        Only the inspected PerformanceDelay variable may be written. Any new
        numeric task rejects; later numerical reads of this variable still lack
        a runtime storage binding and cannot silently become zero.
        """
        if name != 'PursuedDamage_PerformanceDelay' or name not in self.templates:
            raise Unsupported(f'unresolved presentation template {name}')
        allowed = {'DefineDynamicValue','SetDynamicValue','Retarget','PredicateTaskList',
                   'PerformDelayExecute','VCameraConfigChange','RadialBlurCurveEffect',
                   'TargetAlias','ByTargetAliveState','BySimulateSpeedUp','ByInTimelinePerform'}
        def inspect(node: object) -> None:
            """Reject hidden numeric tasks in every branch and delayed task list."""
            if isinstance(node, dict):
                kind = node.get('$type', '').removeprefix('RPG.GameCore.')
                if kind and kind not in allowed:
                    raise Unsupported(f'non-presentation template node {kind}')
                if kind in {'DefineDynamicValue','SetDynamicValue'} and node.get('DynamicKey') != {'Value':'MDF_PerformanceDelay'}:
                    raise Unsupported('presentation template writes a numerical runtime key')
                for child in node.values(): inspect(child)
            elif isinstance(node, list):
                for child in node: inspect(child)
        inspect(self.templates[name])

    def reference_hash(self, hashed: str) -> dict:
        """Resolve a hash without merging transient and persistent namespaces."""
        if hashed not in self.runtime_keys:
            raise Unsupported(f'undeclared runtime storage for dynamic hash {hashed}')
        binding = self.runtime_keys[hashed]
        if isinstance(binding, str) and binding:
            return dict(op='variable', key=binding)
        if (isinstance(binding, dict) and set(binding) == {'scope','key'}
                and binding['scope'] in {'ability','task_template'}
                and isinstance(binding['key'], str) and binding['key']):
            return dict(op='scoped_variable', **binding)
        raise Unsupported(f'invalid explicit runtime storage binding for {hashed}')

    def reference(self, name: str) -> dict:
        """Resolve a named runtime read; each caller supplies its source scope."""
        return self.reference_hash(stable_hash(name))

    def write(self, name: str, value: dict, raw_scope: str | None) -> dict:
        """Require explicit source scope annotations to match the binding."""
        reference = self.reference(name)
        scope = reference.get('scope')
        if raw_scope is not None and raw_scope != {'ability':'ContextAbility','task_template':'ContextTaskTemplate'}.get(scope):
            raise Unsupported(f'ContextScope {raw_scope} does not match explicit storage binding for {name}')
        if scope is None:
            return dict(type='set_dynamic_value', key=reference['key'], value=value, target='self_unit')
        return dict(type='set_scoped_dynamic_value', scope=scope, key=reference['key'], value=value)

    def expression(self, value: dict) -> dict:
        """Bind immutable parameters and scope every remaining dynamic load."""
        try:
            result = compile_expression(value, self.compiler.bindings)
        except (ValueError, KeyError, TypeError) as error:
            raise Unsupported(str(error)) from error

        def scope(node: object) -> None:
            """Rewrite live variable references without changing numeric constants."""
            if isinstance(node, dict):
                if node.get('op') == 'variable':
                    reference = self.reference_hash(node['key'])
                    node.clear()
                    node.update(reference)
                else:
                    for child in node.values():
                        scope(child)
        scope(result)
        return result

    @staticmethod
    def fields(node: dict, allowed: set[str]):
        """Reject unhandled fields, even on otherwise supported numeric tasks."""
        extra = set(node) - allowed - {'$type'}
        if extra:
            raise Unsupported(f'unhandled fields {sorted(extra)}')
        if 'ContextScope' in node and ('ContextScope' in allowed) and node['ContextScope'] not in ('ContextAbility','ContextTaskTemplate'):
            raise Unsupported('unsupported explicit ContextScope')

    @staticmethod
    def owner_target(node: dict):
        """Only explicitly owner-scoped callbacks are accepted by this lowering."""
        if 'TargetType' in node and node['TargetType'] not in [
            {'$type': 'RPG.GameCore.TargetAlias', 'Alias': 'Caster'},
            {'$type': 'RPG.GameCore.TargetAlias', 'Alias': 'ModifierOwnerEntity'},
        ]:
            raise Unsupported('runtime callback target is not its declared owner')

    def condition(self, node: dict) -> dict:
        """Compile live comparisons; never fold an unknown runtime branch away."""
        kind = node['$type'].removeprefix('RPG.GameCore.')
        if kind == 'ByCurrentSkillType':
            self.fields(node, {'SkillType', 'Inverse'})
            kinds = {'Normal': 'basic', 'Skill': 'skill', 'Ultra': 'ultimate'}
            if node['SkillType'] not in kinds:
                raise Unsupported(f'unsupported SkillType {node["SkillType"]}')
            result = dict(type='action_kind', kind=kinds[node['SkillType']])
        elif kind == 'ByCompareDynamicValue':
            self.fields(node, {'TargetType', 'DynamicKey', 'CompareType', 'CompareValue', 'Inverse'})
            self.owner_target(node)
            comparisons = {'Equal': 'equal', 'NotEqual': 'not_equal', 'Greater': 'greater',
                           'GreaterEqual': 'greater_equal', 'Less': 'less', 'LessEqual': 'less_equal'}
            if node['CompareType'] not in comparisons:
                raise Unsupported(f'unsupported comparison {node["CompareType"]}')
            result = dict(type='compare', left=self.reference(node['DynamicKey']['Value']),
                          comparison=comparisons[node['CompareType']], right=self.expression(node['CompareValue']))
        else:
            raise Unsupported(f'runtime predicate {kind}')
        inverse = node.get('Inverse', False)
        if not isinstance(inverse, bool):
            raise Unsupported('predicate Inverse must be boolean')
        return dict(type='not', condition=result) if inverse else result

    def tasks(self, tasks: list[dict], pointer: str, start_index: int = 0) -> list[dict]:
        """Lower both branches; start_index preserves offsets of selected slices."""
        effects = []
        for index, task in enumerate(tasks, start_index):
            where = f'{pointer}/{index}'
            kind = task['$type'].removeprefix('RPG.GameCore.')
            row = dict(pointer=where, type=task['$type'])
            self.compiler.evidence.append(row)
            try:
                if kind == 'PredicateTaskList':
                    self.fields(task, {'Predicate', 'SuccessTaskList', 'FailedTaskList'})
                    effects.append(dict(type='conditional', condition=self.condition(task['Predicate']),
                        then_effects=self.tasks(task.get('SuccessTaskList', []), where+'/SuccessTaskList'),
                        else_effects=self.tasks(task.get('FailedTaskList', []), where+'/FailedTaskList')))
                    row['handling'] = 'runtime_branch_both_paths_checked'
                elif kind == 'SetDynamicValue':
                    self.fields(task, {'Value', 'DynamicKey', 'TargetType', 'ContextScope'})
                    self.owner_target(task)
                    effects.append(self.write(task['DynamicKey']['Value'], self.expression(task['Value']), task.get('ContextScope')))
                    row['handling'] = 'scoped_runtime_write'
                elif kind == 'SetDynamicValueByAddValue':
                    self.fields(task, {'Key','AddValue','TargetType','ContextScope'})
                    self.owner_target(task)
                    name = task['Key']['Value']
                    value = dict(op='add', left=self.reference(name), right=self.expression(task['AddValue']))
                    effects.append(self.write(name, value, task.get('ContextScope')))
                    row['handling'] = 'scoped_runtime_add'
                elif kind == 'DefineDynamicValue':
                    self.fields(task, {'DynamicKey','ResetValue','TargetType','ContextScope'})
                    self.owner_target(task)
                    if 'ResetValue' not in task:
                        raise Unsupported('DefineDynamicValue lacks explicit ResetValue; native default is unresolved')
                    effects.append(self.write(task['DynamicKey']['Value'], self.expression(task['ResetValue']), task.get('ContextScope')))
                    row['handling'] = 'explicit_runtime_initialization'
                elif kind == 'LoopExecuteTaskList':
                    self.fields(task, {'MaxLoopCount', 'TaskList'})
                    effects.append(dict(type='repeat', count=self.expression(task['MaxLoopCount']),
                                        effects=self.tasks(task['TaskList'], where+'/TaskList')))
                    row['handling'] = 'bounded_runtime_loop'
                elif kind == 'ModifyGridFightCustomProperty':
                    self.fields(task, {'CustomType', 'ModifyValue'})
                    custom = task['CustomType']
                    if custom.get('EnumIndex') != 2 or set(custom) != {'EnumIndex', 'Value'}:
                        raise Unsupported('unsupported custom property domain')
                    key = f'grid_fight_custom:{custom["Value"]}'
                    effects.append(dict(type='set_dynamic_value', key=key, target='self_unit',
                        value=dict(op='add', left=dict(op='variable', key=key), right=self.expression(task['ModifyValue']))))
                    row['handling'] = 'persistent_value_output_no_account_write'
                elif kind == 'SetEnergyBarState':
                    self.fields(task, {'Active', 'BarType', 'CurrentState', 'IconPath', 'CurrentCount'})
                    # HUD state is presentation. Validate the numeric expression anyway.
                    self.expression(task['CurrentCount'])
                    row['handling'] = 'presentation_only_energy_bar'
                elif kind == 'Retarget':
                    self.fields(task, {'TargetType', 'ByRandom', 'MaxNumber', 'TaskList', 'FailedTaskList'})
                    if task['TargetType'] != {'$type':'RPG.GameCore.TargetAlias','Alias':'AttackTargetList'} or task.get('ByRandom') is not True or self.compiler.number(task['MaxNumber']) != 1:
                        raise Unsupported('only one random prior attack target is implemented')
                    effects.append(dict(type='retarget_attack_random', effects=self.tasks(task['TaskList'], where+'/TaskList'),
                                        empty_effects=self.tasks(task.get('FailedTaskList', []), where+'/FailedTaskList')))
                    row['handling'] = 'live_prior_attack_target_list'
                elif kind == 'IncludeTaskListTemplate':
                    self.fields(task, {'Name','ParamTarget'})
                    if task.get('ParamTarget') != {'$type':'RPG.GameCore.TargetAlias','Alias':'ParamEntity'}:
                        raise Unsupported('presentation template target is not current retarget context')
                    self.presentation_delay_template(task['Name'])
                    row['handling'] = 'inspected_presentation_delay_zero_av'
                elif kind == 'DamageByAttackProperty':
                    self.fields(task, {'TargetType','AttackProperty','CanTriggerLastKill','PerformanceDelay','IsHitAnimPerformanceDelay'})
                    if task['TargetType'] != {'$type':'RPG.GameCore.TargetAlias','Alias':'ParamEntity'}:
                        raise Unsupported('runtime pursued hit requires retarget context')
                    attack = task['AttackProperty']
                    self.fields(attack, {'DamageType','DamagePercentage','AttackType','HitAnimation','HitEffect',
                        'HitEffectMinMutexTime','HitEffectMaxMutexCount','HitEffectHeight','HitPosHeight','RandomHitAngleHorizontal'})
                    if attack.get('$type') != 'RPG.GameCore.AttackData' or attack.get('AttackType') != 'Pursued':
                        raise Unsupported('runtime damage supports the sourced Pursued hit contract only')
                    if task.get('CanTriggerLastKill') is not True:
                        raise Unsupported('pursued death attribution requires explicit CanTriggerLastKill=true')
                    if attack.get('DamageType') != {'SourceType':'Random'}:
                        raise Unsupported('pursued damage element source is unsupported')
                    # Damage text/animation delay is zero AV under the atomic Action
                    # contract; never reuse this field as a damage/resource input.
                    if 'PerformanceDelay' in task and task['PerformanceDelay'] != {'IsDynamic':True,
                        'PostfixExpr':{'OpCodes':'AQAR','FixedValues':[],'DynamicHashes':[1854514093]}}:
                        raise Unsupported('unrecognized pursued performance delay')
                    effects.append(dict(type='elemental_damage', scale='attack', ratio=self.expression(attack['DamagePercentage']),
                        flat=dict(op='constant', value=0), element=dict(selection='random_all'), attack_type='pursued', can_crit=True, target='context_target'))
                    row['handling'] = 'random_element_pursued_hit_explicit_panels'
                    row['raw_attack_type'] = attack['AttackType']
                else:
                    raise Unsupported(f'runtime task {kind}')
            except (Unsupported, KeyError, TypeError) as error:
                row['handling'] = 'rejected'
                raise Unsupported(f'{where}: {error}') from error
        return effects

    def property_changes(self, definition: dict, pointer: str) -> list[dict]:
        """Lower pure OnStack properties, leaving lifecycle/stack policy explicit.

        Property names cross-checked with HSR_Nous 00185ddc's MIT-licensed
        enemy_ability._compile_property_modifier. Its inferred defaults and
        unconsumed callback behavior are deliberately not imported here.
        """
        self.fields(definition, {'_CallbackList'})
        changes, seen = [], set()
        properties = {'CriticalDamageBase': 'crit_damage', 'AllDamageTypeAddedRatio': 'damage_bonus'}
        for i, callback in enumerate(definition.get('_CallbackList', [])):
            where = f'{pointer}/_CallbackList/{i}'
            self.fields(callback, {'Event', 'CallbackConfig'})
            if callback.get('Event') != 'OnStack':
                raise Unsupported(f'{where}: unsupported property modifier callback {callback.get("Event")}')
            for j, task in enumerate(callback['CallbackConfig']):
                location = f'{where}/CallbackConfig/{j}'
                self.fields(task, {'TargetType', 'Property', 'PropertyValue'})
                self.owner_target(task)
                if task.get('$type') != 'RPG.GameCore.StackProperty' or task.get('Property') not in properties:
                    raise Unsupported(f'{location}: unsupported modifier property task')
                prop = properties[task['Property']]
                if prop in seen:
                    raise Unsupported(f'{location}: repeated property composition is unresolved')
                seen.add(prop)
                changes.append(dict(property=prop, value=self.expression(task['PropertyValue'])))
                self.compiler.evidence.append(dict(pointer=location, type=task['$type'], handling='property_contribution_requires_explicit_modifier_policy'))
        return changes
