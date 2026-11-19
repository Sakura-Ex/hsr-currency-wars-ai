"""Lower explicitly selected StackWeakness leaf tasks, never a whole Modifier.

The caller supplies its modifier instance policy and records excluded callbacks.
Missing defaults, resistance tasks, random choices and control are not inferred.
"""
from .errors import Unsupported

ELEMENTS = {'Physical':'physical','Fire':'fire','Ice':'ice','Thunder':'thunder',
            'Wind':'wind','Quantum':'quantum','Imaginary':'imaginary'}


def weakness_attachments(tasks: list[dict], pointer: str, start_index: int = 0) -> dict:
    """Compile only explicit owner-bound Attach leaves with exact source offsets."""
    if not tasks:
        raise Unsupported('empty StackWeakness task selection')
    elements, evidence = [], []
    for index, task in enumerate(tasks, start_index):
        where = f'{pointer}/{index}'
        try:
            if set(task) != {'$type','TargetType','OPType','WeakList'}:
                raise Unsupported('StackWeakness requires exactly the inspected explicit fields')
            if task['$type'] != 'RPG.GameCore.StackWeakness' or task['OPType'] != 'Attach':
                raise Unsupported('only StackWeakness Attach leaves are supported')
            if task['TargetType'] != {'$type':'RPG.GameCore.TargetAlias','Alias':'ModifierOwnerEntity'}:
                raise Unsupported('StackWeakness target is not the modifier owner')
            values = task['WeakList']
            if not isinstance(values,list) or not values or any(not isinstance(v,str) or v not in ELEMENTS for v in values):
                raise Unsupported('invalid explicit weakness list')
            for value in values:
                element = ELEMENTS[value]
                if element in elements:
                    raise Unsupported('repeated weakness contribution in selected leaves')
                elements.append(element)
            evidence.append(dict(pointer=where,type=task['$type'],handling='modifier_owned_attachment_only',elements=[ELEMENTS[v] for v in values]))
        except Unsupported as error:
            raise Unsupported(f'{where}: {error}') from error
    return dict(scope='selected_stack_weakness_leaves_only',elements=elements,operations=evidence)
