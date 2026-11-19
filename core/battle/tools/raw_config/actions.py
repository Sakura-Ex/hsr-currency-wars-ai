"""Compile selected raw Config skill chains into battle_v2 effects.

This operates on explicitly selected skills and resolved combat panels. It never
certifies a complete character kit: passive/AI/stage imports are separately
attempted and unsupported tasks raise with their exact source JSON pointer.
"""
from __future__ import annotations

import base64
import collections
import json
import math
from pathlib import Path
from . import schema
from .errors import Unsupported
from .expression import stable_hash

# These tasks only control camera/model/animation. Their ordering is recorded;
# numeric tasks are never accepted by a name prefix or silently discarded.
PRESENTATION = {
    'LookAt', 'TriggerAnimStateWithMove', 'TriggerAnimState', 'RadialBlurEffect',
    'MoveToTargetPosition', 'VCameraConfigChange', 'StartAim', 'StopAim',
}
WAITS = {'WaitAnimState', 'WaitSecond', 'WaitTimelineFinish'}
MARKERS = {'DamagePerformFinish', 'SkillPerformFinish', 'SkillExecutionStart'}


class Compiler:
    """Resolve source abilities and lower one regular skill, preserving order.

    Attributes:
        bindings: Explicit dynamic hash values from Config/Excel or caller state.
        points: Explicit active trace-point names.
        evidence: Ordered source pointers and handling decisions.
    """

    def __init__(self, files: list[Path], bindings: dict[str, float], points: list[str], hit_energy: float):
        """Load raw ability definitions; duplicate names are an input error."""
        self.abilities = {}
        self.modifiers = {}
        self.modifier_pointers = {}
        self.bindings = bindings
        self.points = set(points)
        if not math.isfinite(hit_energy) or hit_energy < 0:
            raise ValueError("hit_energy must be nonnegative and finite")
        self.hit_energy = hit_energy
        self.evidence = []
        self.stack = []
        self.types = collections.Counter()
        for path in files:
            data = json.loads(path.read_text(encoding='utf-8-sig'))
            self.scan(data)
            for i, ability in enumerate(data['AbilityList']):
                name = ability['Name']
                if name in self.abilities:
                    raise ValueError(f'duplicate ability {name}')
                self.abilities[name] = (ability, f'{path.as_posix()}#/AbilityList/{i}')
                for key, modifier in ability.get('Modifiers', {}).items():
                    self.modifiers[(name,key)] = modifier
                    self.modifier_pointers[(name,key)] = schema.child(f'{path.as_posix()}#/AbilityList/{i}/Modifiers', key)

    def scan(self, value: object) -> None:
        """Count every typed node, including branches not selected for execution."""
        if isinstance(value, dict):
            if '$type' in value:
                self.types[value['$type']] += 1
            for child in value.values():
                self.scan(child)
        elif isinstance(value, list):
            for child in value:
                self.scan(child)

    def number(self, value: dict, pointer: str = 'number', bindings: dict | None = None) -> float:
        """Resolve a strictly shaped numeric wrapper with located failure evidence.

        Args:
            value: Supported literal, fixed or postfix number.
            pointer: Exact source location of this expression.
            bindings: Optional explicit local preshow parameters; never global writes.

        Raises:
            Unsupported: Unknown fields/opcodes, missing binding or invalid stack.
        """
        try:
            schema.number(value, pointer)
            resolved = self.bindings if bindings is None else bindings
            if 'Value' in value:
                return float(value['Value'])
            if value['IsDynamic'] is False:
                return float(value['FixedValue']['Value'])
            expr = value['PostfixExpr']
            codes = base64.b64decode(expr['OpCodes'], validate=True)
            if not codes or codes[-1] != 17:
                raise Unsupported('postfix expression lacks terminator')
            stack = []
            index = 0
            while index < len(codes):
                op = codes[index]
                index += 1
                if op in (0, 1):
                    operand = codes[index]
                    index += 1
                    if op == 0:
                        stack.append(expr['FixedValues'][operand]['Value'])
                    else:
                        key = str(expr['DynamicHashes'][operand])
                        if key not in resolved:
                            raise Unsupported(f'unresolved dynamic hash {key}')
                        stack.append(resolved[key])
                elif op == 14:
                    stack.append(-stack.pop())
                elif op == 17 and index == len(codes):
                    break
                else:
                    raise Unsupported(f'unsupported postfix opcode {op}')
            if len(stack) != 1:
                raise Unsupported('invalid postfix stack')
            schema.finite(stack[0], pointer)
            return float(stack[0])
        except (ValueError, IndexError, KeyError, TypeError) as error:
            message = str(error)
            if message.startswith(pointer + '/') or message.startswith(pointer + ':'):
                raise Unsupported(message) from error
            raise Unsupported(f'{pointer}/PostfixExpr/OpCodes: {message}') from error

    def predicate(self, node: dict, pointer: str = 'predicate') -> bool:
        """Evaluate only declared regular-context predicates, rejecting extra fields."""
        try:
            schema.fields(node, {'$type', 'PointTriggerKey', 'TargetType', 'Inverse'}, pointer, {'$type'})
            schema.text(node['$type'], pointer + '/$type')
            kind = node['$type'].removeprefix('RPG.GameCore.')
            if node['$type'] != 'RPG.GameCore.' + kind:
                raise Unsupported(f'{pointer}/$type: unsupported predicate namespace')
            allowed = {
                'BySkillPointActivated': {'PointTriggerKey', 'Inverse'},
                'ByIsInsertAction': {'Inverse'},
                'ByIsTurnOwnerEntity': {'TargetType', 'Inverse'},
            }
            if kind not in allowed:
                raise Unsupported(f'{pointer}: predicate {kind}')
            schema.fields(node, allowed[kind] | {'$type'}, pointer)
            if kind == 'BySkillPointActivated':
                schema.fields(node, allowed[kind] | {'$type'}, pointer, {'PointTriggerKey'})
                schema.text(node['PointTriggerKey'], pointer + '/PointTriggerKey')
                result = node['PointTriggerKey'] in self.points
            elif kind == 'ByIsInsertAction':
                result = False
            else:
                schema.fields(node, allowed[kind] | {'$type'}, pointer, {'TargetType'})
                schema.alias(node['TargetType'], pointer + '/TargetType', {'Caster'})
                result = True
            inverse = node.get('Inverse', False)
            schema.boolean(inverse, pointer + '/Inverse')
            return not result if inverse else result
        except ValueError as error:
            raise Unsupported(str(error)) from error

    def preshow_modifier(self, task: dict, pointer: str) -> None:
        """Validate the inspected self-only action-delay preview modifier contract.

        Numeric callbacks are rejected. Local dynamic values must match declared
        preview hashes and are evaluated only for validating the preview data.
        """
        schema.fields(task, {'$type', 'ModifierName', 'TargetType', 'DynamicValues'}, pointer,
                      {'ModifierName', 'TargetType'})
        schema.alias(task['TargetType'], pointer + '/TargetType', {'Caster'})
        key = schema.name(task['ModifierName'], pointer + '/ModifierName')
        scoped = [(owner, key) for owner in reversed(self.stack) if (owner, key) in self.modifiers]
        if not scoped:
            raise Unsupported(f'{pointer}/ModifierName: unresolved modifier {key}')
        modifier = self.modifiers[scoped[0]]
        where = self.modifier_pointers[scoped[0]]
        schema.fields(modifier, {'ModifierAffectedPreshowConfig', '_CallbackList', 'Stacking', 'DynamicValues'}, where,
                      {'ModifierAffectedPreshowConfig'})
        if 'Stacking' in modifier and modifier['Stacking'] != 'Multiple':
            raise Unsupported(f'{where}/Stacking: unsupported preshow stacking')
        callbacks = modifier.get('_CallbackList', [])
        schema.sequence(callbacks, where + '/_CallbackList')
        for index, callback in enumerate(callbacks):
            location = f'{where}/_CallbackList/{index}'
            schema.fields(callback, {'Event', 'CallbackConfig'}, location, {'Event', 'CallbackConfig'})
            if callback['Event'] != 'OnActionEnd':
                raise Unsupported(f'{location}/Event: numeric modifier callback {callback["Event"]}')
            schema.sequence(callback['CallbackConfig'], location + '/CallbackConfig')
            if len(callback['CallbackConfig']) != 1:
                raise Unsupported(f'{location}/CallbackConfig: expected one RemoveSelfModifier')
            removal = callback['CallbackConfig'][0]
            schema.fields(removal, {'$type'}, location + '/CallbackConfig/0', {'$type'})
            if removal['$type'] != 'RPG.GameCore.RemoveSelfModifier':
                raise Unsupported(f'{location}/CallbackConfig/0/$type: only RemoveSelfModifier is inspected')
        declared = modifier.get('DynamicValues', {'Floats': {}})
        schema.fields(declared, {'Floats'}, where + '/DynamicValues', {'Floats'})
        floats = declared['Floats']
        if not isinstance(floats, dict):
            raise Unsupported(f'{where}/DynamicValues/Floats: expected object')
        for hashed, declaration in floats.items():
            location = schema.child(where + '/DynamicValues/Floats', hashed)
            schema.fields(declaration, {'ReadInfo'}, location, {'ReadInfo'})
            schema.fields(declaration['ReadInfo'], {'Type', 'Index'}, location + '/ReadInfo', {'Type', 'Index'})
            if (declaration['ReadInfo'] != {'Type': 'None', 'Index': 0}
                    or type(declaration['ReadInfo']['Index']) is not int):
                raise Unsupported(f'{location}/ReadInfo: unsupported preshow storage declaration')
        supplied = task.get('DynamicValues', {})
        if not isinstance(supplied, dict):
            raise Unsupported(f'{pointer}/DynamicValues: expected object')
        local = dict(self.bindings)
        for name, value in supplied.items():
            location = schema.child(pointer + '/DynamicValues', name)
            hashed = stable_hash(name)
            if hashed not in floats:
                raise Unsupported(f'{location}: undeclared preshow dynamic value')
            local[hashed] = self.number(value, location)
        if set(floats) != {stable_hash(name) for name in supplied}:
            raise Unsupported(f'{pointer}/DynamicValues: every preshow storage value must be explicitly supplied')
        preview = modifier['ModifierAffectedPreshowConfig']
        location = where + '/ModifierAffectedPreshowConfig'
        schema.fields(preview, {'SkillTypes', 'TargetType', 'ActionDelayPreshowConfig'}, location,
                      {'SkillTypes', 'TargetType', 'ActionDelayPreshowConfig'})
        if preview['SkillTypes'] != ['Normal']:
            raise Unsupported(f'{location}/SkillTypes: unsupported preshow skill types')
        schema.alias(preview['TargetType'], location + '/TargetType', {'ModifierOwnerEntity'})
        delay = preview['ActionDelayPreshowConfig']
        schema.fields(delay, {'AddDelayRatio'}, location + '/ActionDelayPreshowConfig', {'AddDelayRatio'})
        self.number(delay['AddDelayRatio'], location + '/ActionDelayPreshowConfig/AddDelayRatio', local)

    def ability(self, name: str) -> list[dict]:
        """Validate ability fields, then lower supported phases and references.

        Only OnAdd and OnStart execute. Nonempty unhandled callbacks/metadata
        reject at the field pointer; unused modifier definitions are declarations
        and are checked when an AddModifier task actually references them.
        """
        if name in self.stack or len(self.stack) >= 32:
            raise Unsupported(f'cyclic ability reference {name}')
        if name not in self.abilities:
            raise Unsupported(f'missing ability {name}')
        ability, pointer = self.abilities[name]
        for field, value in ability.items():
            where = pointer + '/' + field.replace('~', '~0').replace('/', '~1')
            reason = None
            if field in {'Name', 'Modifiers'}:
                continue
            if field in {'OnAdd', 'OnStart'}:
                if not isinstance(value, list):
                    reason = 'ability phase must be a task list'
            elif field == 'TargetInfo':
                if value != {'TargetType': 'SkillTargetEntityList'}:
                    reason = 'unsupported ability target'
            elif value is None or value == [] or value == {}:
                self.evidence.append({'pointer': where, 'field': field, 'handling': 'empty_unsupported_field'})
            else:
                reason = 'unsupported ability field'
            if reason:
                self.evidence.append({'pointer': where, 'field': field, 'handling': 'rejected'})
                raise Unsupported(f'{where}: {reason} {field}')
        self.stack.append(name)
        try:
            effects = []
            for phase in ['OnAdd', 'OnStart']:
                effects += self.tasks(ability.get(phase, []), pointer + '/' + phase)
            return effects
        finally:
            self.stack.pop()

    def tasks(self, tasks: list[dict], pointer: str) -> list[dict]:
        """Lower selected tasks with closed field schemas and located rejection."""
        try:
            schema.sequence(tasks, pointer)
        except ValueError as error:
            raise Unsupported(str(error)) from error
        effects = []
        for i, task in enumerate(tasks):
            where = f'{pointer}/{i}'
            row = {'pointer': where}
            self.evidence.append(row)
            try:
                if not isinstance(task, dict) or not isinstance(task.get('$type'), str):
                    raise Unsupported(f'{where}/$type: task requires a string type')
                kind = task['$type'].removeprefix('RPG.GameCore.')
                if task['$type'] != 'RPG.GameCore.' + kind:
                    raise Unsupported(f'{where}/$type: unsupported task namespace')
                row['type'] = task['$type']
                if kind in PRESENTATION | WAITS | MARKERS:
                    schema.presentation(kind, task, where)
                    row['handling'] = 'presentation_only' if kind in PRESENTATION else 'ordered_marker_zero_av'
                elif kind == 'TriggerAbility':
                    schema.fields(task, {'$type', 'AbilityName', 'TargetType', 'IsSkillPerform'}, where,
                                  {'AbilityName', 'TargetType'})
                    schema.alias(task['TargetType'], where + '/TargetType', {'Caster'})
                    name = schema.name(task['AbilityName'], where + '/AbilityName')
                    if 'IsSkillPerform' in task and task['IsSkillPerform'] is not True:
                        raise Unsupported(f'{where}/IsSkillPerform: only explicit true or legacy omission is supported')
                    row['handling'] = 'resolve_reference'
                    effects += self.ability(name)
                elif kind == 'PredicateTaskList':
                    schema.fields(task, {'$type', 'Predicate', 'SuccessTaskList', 'FailedTaskList'}, where, {'Predicate'})
                    for field in ['SuccessTaskList', 'FailedTaskList']:
                        schema.sequence(task.get(field, []), where + '/' + field)
                    branch = 'SuccessTaskList' if self.predicate(task['Predicate'], where + '/Predicate') else 'FailedTaskList'
                    row['handling'] = branch
                    skipped = 'FailedTaskList' if branch == 'SuccessTaskList' else 'SuccessTaskList'
                    self.evidence.append({'pointer': where + '/' + skipped,
                        'handling': 'not_selected_declared_regular_context', 'task_count': len(task.get(skipped, []))})
                    effects += self.tasks(task.get(branch, []), where + '/' + branch)
                elif kind == 'DamageByAttackProperty':
                    if 'CanTriggerLastKill' in task and task['CanTriggerLastKill'] is not True:
                        raise Unsupported(f'{where}/CanTriggerLastKill: unimplemented CanTriggerLastKill value; only explicit true was inspected')
                    schema.fields(task, {'$type', 'TargetType', 'AttackProperty', 'SpecialHitSoundEvent', 'CanTriggerLastKill'}, where,
                                  {'TargetType', 'AttackProperty'})
                    row['can_trigger_last_kill'] = 'explicit_true' if 'CanTriggerLastKill' in task else 'omitted_native_default_unresolved_legacy_direct_hit_contract'
                    schema.alias(task['TargetType'], where + '/TargetType', {'AbilityTargetEntity'})
                    if 'SpecialHitSoundEvent' in task:
                        schema.text(task['SpecialHitSoundEvent'], where + '/SpecialHitSoundEvent')
                    attack = task['AttackProperty']
                    location = where + '/AttackProperty'
                    allowed = {'$type', 'DamageType', 'DamagePercentage', 'StanceValue', 'HitSplitRatio',
                               'SPHitRatio', 'HitAnimation', 'HitEffect', 'HitEffectHeight', 'HitTimeSlowType',
                               'HitTimeSlowIntensity', 'HitPosHeight', 'HitAngleHorizontal', 'HitAngleVertical'}
                    schema.fields(attack, allowed, location, {'$type', 'DamageType', 'DamagePercentage'})
                    if attack['$type'] != 'RPG.GameCore.AttackData':
                        raise Unsupported(f'{location}/$type: expected AttackData')
                    schema.fields(attack['DamageType'], {'DamageType'}, location + '/DamageType', {'DamageType'})
                    if attack['DamageType']['DamageType'] not in {'Physical', 'Fire', 'Ice', 'Thunder', 'Wind', 'Quantum', 'Imaginary'}:
                        raise Unsupported(f'{location}/DamageType/DamageType: unsupported element')
                    for field in ['HitAnimation', 'HitEffect', 'HitTimeSlowType', 'HitTimeSlowIntensity']:
                        if field in attack: schema.text(attack[field], location + '/' + field)
                    for field in ['HitEffectHeight', 'HitPosHeight', 'HitAngleHorizontal', 'HitAngleVertical']:
                        if field in attack: schema.finite(attack[field], location + '/' + field)
                    hit_energy_ratio = self.number(attack['SPHitRatio'], location + '/SPHitRatio') if 'SPHitRatio' in attack else 1.0
                    # Unsplit-hit default is the existing explicit legacy convention.
                    split = self.number(attack['HitSplitRatio'], location + '/HitSplitRatio') if 'HitSplitRatio' in attack else 1.0
                    ratio = self.number(attack['DamagePercentage'], location + '/DamagePercentage') * split
                    effects.append(dict(type='damage', scale='attack', ratio=ratio, flat=0,
                                        hits=1, can_crit=True, target='ability_target'))
                    if 'StanceValue' in attack:
                        effects.append(dict(type='toughness', amount=self.number(attack['StanceValue'], location + '/StanceValue') * split,
                                            target='ability_target'))
                    effects.append(dict(type='energy', amount=self.hit_energy * hit_energy_ratio, target='ability_target'))
                    row.update(handling='ordered_damage_hit', ratio=ratio, hit_split=split, hit_energy=self.hit_energy * hit_energy_ratio,
                               element=attack['DamageType']['DamageType'])
                elif kind == 'ModifyCurrentSkillDelayCost':
                    schema.fields(task, {'$type', 'NormalizedValue', 'TargetType'}, where, {'NormalizedValue'})
                    if 'TargetType' in task: schema.alias(task['TargetType'], where + '/TargetType', {'Caster'})
                    effects.append(dict(type='advance_next', fraction=-self.number(task['NormalizedValue'], where + '/NormalizedValue'), target='self_unit'))
                    row['handling'] = 'turn_end_next_distance'
                elif kind == 'AddModifier':
                    self.preshow_modifier(task, where)
                    row['handling'] = 'preshow_only_modifier'
                else:
                    raise Unsupported(f'task {kind}')
            except (KeyError, IndexError, ValueError, TypeError) as error:
                row['handling'] = 'rejected'
                message = str(error)
                raise Unsupported(message if message.startswith(where + '/') or message.startswith(where + ':')
                                  else f'{where}: {message}') from error
        return effects


def bindings_from_config(config: dict, skills: dict[str, dict], points: dict[str, dict]) -> dict[str, float]:
    """Resolve Config dynamic hashes through exact selected Excel ParamList rows."""
    result = {}
    for key, field in config['DynamicValues']['Floats'].items():
        read = field['ReadInfo']
        table = skills if read['Type'] == 'SkillParam' else points if read['Type'] == 'SkillTreeParam' else {}
        if read.get('TriggerKey') in table:
            result[key] = table[read['TriggerKey']]['ParamList'][read['Index']]['Value']
    return result
