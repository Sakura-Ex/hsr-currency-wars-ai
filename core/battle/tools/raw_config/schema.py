"""Closed field shapes for the selected Config lowerings, not a native interpreter.

Every accepted object has named consumed or inspected presentation fields.
Unknown fields reject at their JSON pointer, including inside display payloads.
"""
import math


def child(pointer: str, key: str | int) -> str:
    """Append one escaped JSON-pointer token."""
    return pointer + '/' + str(key).replace('~', '~0').replace('/', '~1')


def fields(node: object, allowed: set[str], pointer: str, required: set[str] = frozenset()) -> None:
    """Require an object and reject every unconsumed field, even empty fields."""
    if not isinstance(node, dict):
        raise ValueError(f'{pointer}: expected object')
    for key in node:
        if key not in allowed:
            raise ValueError(f'{child(pointer, key)}: unhandled field')
    for key in sorted(required - node.keys()):
        raise ValueError(f'{child(pointer, key)}: missing required field')


def sequence(value: object, pointer: str) -> None:
    """Require a JSON list without interpreting or silently coercing its items."""
    if not isinstance(value, list):
        raise ValueError(f'{pointer}: expected list')


def text(value: object, pointer: str) -> None:
    """Require a string; nested task objects are never display strings."""
    if not isinstance(value, str):
        raise ValueError(f'{pointer}: expected string')


def boolean(value: object, pointer: str) -> None:
    """Require an actual JSON boolean."""
    if not isinstance(value, bool):
        raise ValueError(f'{pointer}: expected boolean')


def finite(value: object, pointer: str) -> None:
    """Require a finite numeric scalar, excluding booleans."""
    if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value):
        raise ValueError(f'{pointer}: expected finite numeric scalar')


def name(value: object, pointer: str) -> str:
    """Validate the exact Value-only string wrapper and return its content."""
    fields(value, {'Value'}, pointer, {'Value'})
    text(value['Value'], child(pointer, 'Value'))
    if not value['Value']:
        raise ValueError(f'{child(pointer, "Value")}: empty name')
    return value['Value']


def alias(value: object, pointer: str, permitted: set[str] | None = None) -> None:
    """Validate TargetAlias; callers constrain semantic target support explicitly."""
    fields(value, {'$type', 'Alias'}, pointer, {'$type', 'Alias'})
    if value['$type'] != 'RPG.GameCore.TargetAlias':
        raise ValueError(f'{child(pointer, "$type")}: expected TargetAlias')
    text(value['Alias'], child(pointer, 'Alias'))
    if permitted is not None and value['Alias'] not in permitted:
        raise ValueError(f'{child(pointer, "Alias")}: unsupported target {value["Alias"]}')


def number(value: object, pointer: str) -> None:
    """Validate literal/fixed/postfix containers and every operand wrapper.

    Bytecode semantics remain in the existing expression compiler. Shape checks
    also cover unused operand entries so they cannot conceal extra payloads.
    """
    if not isinstance(value, dict):
        raise ValueError(f'{pointer}: expected numeric expression object')
    if 'Value' in value:
        fields(value, {'Value'}, pointer)
        finite(value['Value'], child(pointer, 'Value'))
    elif value.get('IsDynamic') is False:
        fields(value, {'IsDynamic', 'FixedValue'}, pointer, {'FixedValue'})
        fixed = value['FixedValue']
        fields(fixed, {'Value'}, child(pointer, 'FixedValue'), {'Value'})
        finite(fixed['Value'], child(child(pointer, 'FixedValue'), 'Value'))
    elif value.get('IsDynamic') is True:
        fields(value, {'IsDynamic', 'PostfixExpr'}, pointer, {'PostfixExpr'})
        where = child(pointer, 'PostfixExpr')
        expr = value['PostfixExpr']
        fields(expr, {'OpCodes', 'FixedValues', 'DynamicHashes'}, where, {'OpCodes', 'FixedValues', 'DynamicHashes'})
        text(expr['OpCodes'], child(where, 'OpCodes'))
        sequence(expr['FixedValues'], child(where, 'FixedValues'))
        sequence(expr['DynamicHashes'], child(where, 'DynamicHashes'))
        for i, item in enumerate(expr['FixedValues']):
            location = child(child(where, 'FixedValues'), i)
            fields(item, {'Value'}, location, {'Value'})
            finite(item['Value'], child(location, 'Value'))
        for i, hashed in enumerate(expr['DynamicHashes']):
            if isinstance(hashed, bool) or not isinstance(hashed, int) or not -2**31 <= hashed < 2**31:
                raise ValueError(f'{child(child(where, "DynamicHashes"), i)}: expected signed 32-bit hash')
    else:
        raise ValueError(f'{child(pointer, "IsDynamic")}: numeric expression requires boolean IsDynamic or literal Value')


def vector(value: object, pointer: str) -> None:
    """Validate a presentation coordinate vector, with omitted axes allowed."""
    fields(value, {'X', 'Y', 'Z'}, pointer)
    for key, item in value.items():
        finite(item, child(pointer, key))


def camera(value: object, pointer: str) -> None:
    """Validate the selected camera state/blend/shake structures recursively."""
    fields(value, {'NormalConfig', 'BlendConfig', 'OverrideShakeConfigV2', 'ShakeTemplateName'}, pointer)
    for key, item in value.items():
        where = child(pointer, key)
        if key == 'ShakeTemplateName':
            text(item, where)
        elif key == 'NormalConfig':
            fields(item, {'CameraState', 'ResetToDefault', 'TemplateName', 'IsLocalOffset',
                          'AnchorOffset', 'AimOffset', 'AimRatio', 'AnchorRatio', 'Dutch'}, where)
            for field, entry in item.items():
                location = child(where, field)
                if field in {'CameraState', 'TemplateName'}:
                    text(entry, location)
                elif field in {'ResetToDefault', 'IsLocalOffset'}:
                    boolean(entry, location)
                elif field in {'AnchorOffset', 'AimOffset'}:
                    vector(entry, location)
                else:
                    finite(entry, location)
        elif key == 'BlendConfig':
            fields(item, {'BlendType', 'BlendTime', 'CustomCurveName'}, where)
            for field, entry in item.items():
                (finite if field == 'BlendTime' else text)(entry, child(where, field))
        elif key == 'OverrideShakeConfigV2':
            fields(item, {'ShakeDir', 'ShakeScale', 'ShakeTime'}, where)
            for field, entry in item.items():
                (vector if field == 'ShakeDir' else finite)(entry, child(where, field))


def moving_ranges(value: object, pointer: str) -> None:
    """Validate inspected animation movement metadata, excluding nested logic."""
    sequence(value, pointer)
    for index, item in enumerate(value):
        where = child(pointer, index)
        fields(item, {'TargetType', 'OffsetTargetDistance', 'NormalizedTimeStart', 'NormalizedTimeEnd', 'IgnoreRadius'}, where)
        for key, entry in item.items():
            validator = alias if key == 'TargetType' else boolean if key == 'IgnoreRadius' else number
            validator(entry, child(where, key))


# Explicit presentation schemas inspected in the selected regular-skill sources.
# These validators only check shape; excluded animation timing remains zero AV.
PRESENTATION_FIELDS = {
    'LookAt': {'TargetType': text},
    'TriggerAnimStateWithMove': {'TargetType': alias, 'AnimStateName': text, 'MovingRangeList': moving_ranges},
    'TriggerAnimState': {'AnimLogicState': text, 'AnimStateName': text, 'NormalizedTimeEnd': number,
                         'NormalizedTimeWait': number, 'NormalizedTransitionDuration': number},
    'RadialBlurEffect': {'TargetType': alias, 'Active': boolean, 'BlurRadius': finite,
                         'BlurStart': finite, 'BlurFeather': finite, 'Iteration': finite},
    'MoveToTargetPosition': {'TargetType': alias, 'IgnoreRadius': boolean, 'OffsetTargetDistance': number},
    'VCameraConfigChange': {'CameraConfig': camera},
    'StartAim': {'TargetType': alias, 'TargetAttachPoint': text},
    'StopAim': {},
    'WaitAnimState': {'TargetType': alias, 'AnimStateName': text, 'NormalizedTimeEnd': number, 'IgnoreStateChangeCheck': boolean},
    'WaitSecond': {'WaitTime': number},
    'WaitTimelineFinish': {'TimelineName': text},
    'DamagePerformFinish': {},
    'SkillPerformFinish': {},
    'SkillExecutionStart': {},
}


def presentation(kind: str, node: dict, pointer: str) -> None:
    """Check every field of one explicitly supported presentation/marker task."""
    validators = PRESENTATION_FIELDS[kind]
    fields(node, set(validators) | {'$type'}, pointer)
    for key, value in node.items():
        if key != '$type':
            validators[key](value, child(pointer, key))
