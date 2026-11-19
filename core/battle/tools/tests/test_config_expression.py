"""Check raw Config dynamic identities and observed state-update expressions."""
import unittest
from tools.raw_config.expression import compile_expression, stable_hash


class ExpressionTests(unittest.TestCase):
    """Verify fixed source identities and reject unsupported numeric instructions."""

    def test_named_dynamic_hashes_match_config_declarations(self):
        """Three independently stored raw hashes must map back to their names."""
        for name, key in [('Skill_Count', '-169675107'), ('Purse_Attack_Count', '-372803990'),
                          ('GridFight_S2_Current_MagicLayer', '-1581744091')]:
            self.assertEqual(stable_hash(name), key)

    def test_observed_increment_and_subtraction_remain_runtime_expressions(self):
        """Anaxa's count update must read live state, not its initial value."""
        increment = {'IsDynamic':True, 'PostfixExpr':{'OpCodes':'AQAAAAIR',
                     'FixedValues':[{'Value':1}], 'DynamicHashes':[-169675107]}}
        self.assertEqual(compile_expression(increment, {}), {'op':'add',
            'left':{'op':'variable','key':'-169675107'}, 'right':{'op':'constant','value':1.0}})
        subtract = {'IsDynamic':True, 'PostfixExpr':{'OpCodes':'AQABAQMR',
                    'FixedValues':[], 'DynamicHashes':[-169675107,551330741]}}
        self.assertEqual(compile_expression(subtract, {'551330741':3.0})['right'], {'op':'constant','value':3.0})

    def test_unknown_opcode_and_malformed_stack_reject(self):
        """Unidentified raw functions and missing operands cannot compile."""
        for code in ['AQABAQECBxABEQ==', 'ARE=', 'AhE=']:
            with self.subTest(code=code), self.assertRaises(ValueError):
                compile_expression({'IsDynamic':True,'PostfixExpr':{'OpCodes':code,
                    'FixedValues':[],'DynamicHashes':[1776456860,-1581744091,-1943279159]}}, {})

    def test_sourced_floor_and_max_function_calls_consume_the_function_reference(self):
        """Check raw unary/binary call forms independently of runtime arithmetic."""
        self.assertEqual(stable_hash('floor'), '1776456860')
        self.assertEqual(stable_hash('max'), '696029444')
        floor = compile_expression({'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQABAQECBRABEQ==',
            'FixedValues':[],'DynamicHashes':[1776456860,-1581744091,-1943279159]}}, {'-1943279159':5.0})
        self.assertEqual(floor, {'op':'floor','value':{'op':'divide',
            'left':{'op':'variable','key':'-1581744091'},'right':{'op':'constant','value':5.0}}})
        maximum = compile_expression({'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQABAQAAAwABEAIR',
            'FixedValues':[{'Value':1},{'Value':0}], 'DynamicHashes':[696029444,-1814241551]}}, {})
        self.assertEqual(maximum, {'op':'maximum', 'left':{'op':'subtract',
            'left':{'op':'variable','key':'-1814241551'},'right':{'op':'constant','value':1.0}},
            'right':{'op':'constant','value':0.0}})

    def test_unknown_or_nonnumeric_function_forms_reject(self):
        """A function opcode cannot call arbitrary values or use the wrong arity."""
        import base64
        for code, hashes in [([1,0,0,0,16,1,17],[1234]),
                             ([1,0,0,0,0,0,16,2,17],[1776456860]),
                             ([0,0,0,0,16,1,17],[])]:
            with self.assertRaisesRegex(ValueError, 'unsupported postfix function'):
                compile_expression({'IsDynamic':True,'PostfixExpr':{
                    'OpCodes':base64.b64encode(bytes(code)).decode(),
                    'FixedValues':[{'Value':1}], 'DynamicHashes':hashes}}, {})
